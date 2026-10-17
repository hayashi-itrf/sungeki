//! Bounded in-memory LRU. Every hit revalidates the current original's bytes.
use crate::{AudioDecodeRuntime, WavError, Waveform, WaveformError, analyze_waveform};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use sungeki_project::AudioProbe;

#[derive(Debug)]
pub enum WaveformCacheError {
    Configuration,
    MissingRuntime,
    Decode(crate::Mp3DecodeError),
    InvalidProbe,
    SourceChanged,
    Cancelled,
    Io(std::io::Error),
    Wav(WavError),
    Analysis(WaveformError),
}
impl From<std::io::Error> for WaveformCacheError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl std::fmt::Display for WaveformCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRuntime => f.write_str("圧縮音声の復号環境が必要です"),
            Self::Decode(e) => e.fmt(f),
            Self::Configuration => f.write_str("波形cacheの上限が不正です"),
            Self::InvalidProbe => f.write_str("元音声のパスまたは検査記録が不正です"),
            Self::SourceChanged => f.write_str("元音声が検査記録から変更されています"),
            Self::Cancelled => f.write_str("波形取得を取り消しました"),
            Self::Io(e) => e.fmt(f),
            Self::Wav(e) => e.fmt(f),
            Self::Analysis(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for WaveformCacheError {}
struct Entry {
    probe: AudioProbe,
    base_frames: u32,
    waveform: Arc<Waveform>,
    bytes: usize,
}
pub struct CachedWaveform {
    pub waveform: Arc<Waveform>,
    pub cache_hit: bool,
}
pub struct WaveformCache {
    entries: VecDeque<Entry>,
    max_bytes: usize,
    max_entries: usize,
    retained_bytes: usize,
}
impl WaveformCache {
    /// Limits accounted payload allocations, excluding LRU/Arc allocator bookkeeping.
    /// A caller holding an Arc can outlive eviction; this is not a process memory limit.
    pub fn new(max_bytes: usize, max_entries: usize) -> Result<Self, WaveformCacheError> {
        if max_bytes == 0 || max_bytes > 128 * 1024 * 1024 || !(1..=64).contains(&max_entries) {
            return Err(WaveformCacheError::Configuration);
        }
        Ok(Self {
            entries: VecDeque::new(),
            max_bytes,
            max_entries,
            retained_bytes: 0,
        })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.retained_bytes = 0;
    }
    /// Resolves no Project locator; callers must supply the resolved original, never a proxy.
    /// Content hash, analysis version and base resolution identify analysis; full probe is also checked.
    pub fn get(
        &mut self,
        path: &Path,
        probe: &AudioProbe,
        base_frames: u32,
        runtime: Option<&AudioDecodeRuntime<'_>>,
        cancel: &AtomicBool,
    ) -> Result<CachedWaveform, WaveformCacheError> {
        if !(1..=65536).contains(&base_frames) {
            return Err(WaveformCacheError::Analysis(WaveformError::Settings));
        }
        let bytes = crate::audio_original::capture(path, probe, cancel)?;
        if let Some(index) = self.entries.iter().position(|e| {
            e.probe == *probe
                && e.base_frames == base_frames
                && e.waveform.version == crate::WAVEFORM_VERSION
        }) {
            let entry = self.entries.remove(index).unwrap();
            let waveform = Arc::clone(&entry.waveform);
            self.entries.push_back(entry);
            return Ok(CachedWaveform {
                waveform,
                cache_hit: true,
            });
        }
        let analyzed = if probe.encoding.is_some() {
            let runtime = runtime.ok_or(WaveformCacheError::MissingRuntime)?;
            let wav =
                crate::compressed_original::decode_captured_file(bytes, probe, runtime, cancel)
                    .map_err(crate::compressed_original::waveform_error)?;
            crate::analyze_decoded_mp3_waveform(&wav, base_frames, cancel)
        } else {
            let wav = crate::compressed_original::decode_captured(bytes, probe, runtime, cancel)
                .map_err(crate::compressed_original::waveform_error)?;
            analyze_waveform(&wav, base_frames, cancel)
        };
        let mut waveform = analyzed.map_err(|e| match e {
            WaveformError::Cancelled => WaveformCacheError::Cancelled,
            e => WaveformCacheError::Analysis(e),
        })?;
        waveform.source_sha256 = probe.sha256.clone();
        let waveform = Arc::new(waveform);
        let encoding_bytes = match &probe.encoding {
            Some(sungeki_project::CompressedAudioEncoding::Mp3 { decoded_sha256, .. }) => {
                decoded_sha256.capacity()
            }
            None => 0,
        };
        let bytes = waveform.allocated_bytes()
            + std::mem::size_of::<Entry>()
            + probe.sha256.capacity()
            + encoding_bytes;
        if bytes <= self.max_bytes {
            while self.entries.len() >= self.max_entries
                || self.retained_bytes + bytes > self.max_bytes
            {
                let old = self.entries.pop_front().unwrap();
                self.retained_bytes -= old.bytes;
            }
            self.entries.push_back(Entry {
                probe: probe.clone(),
                base_frames,
                waveform: Arc::clone(&waveform),
                bytes,
            });
            self.retained_bytes += bytes;
        }
        Ok(CachedWaveform {
            waveform,
            cache_hit: false,
        })
    }
}
