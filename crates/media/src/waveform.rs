//! Source-sample waveform, before gain/envelope/ducking/master processing.
use crate::{VerifiedWav, WavError};
use std::sync::atomic::{AtomicBool, Ordering};

pub const WAVEFORM_VERSION: u32 = 1;
const MAX_BUCKETS: u64 = 262_144;
const MAX_WINDOW_BUCKETS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePeak {
    pub min: f64,
    pub max: f64,
}
#[derive(Debug)]
pub struct WaveformLevel {
    pub frames_per_bucket: u64,
    channels: Vec<Vec<SamplePeak>>,
}
impl WaveformLevel {
    pub fn bucket_count(&self) -> usize {
        self.channels[0].len()
    }
}
#[derive(Debug)]
pub struct Waveform {
    pub version: u32,
    pub source_sha256: String,
    pub source_frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    /// Per-channel linear RMS and sample peak; neither LUFS nor true peak.
    pub rms: Vec<f64>,
    pub sample_peak: Vec<f64>,
    levels: Vec<WaveformLevel>,
}
#[derive(Debug)]
pub struct WaveformWindow<'a> {
    pub start_frame: u64,
    pub end_frame: u64,
    pub frames_per_bucket: u64,
    pub peaks: &'a [SamplePeak],
}
#[derive(Debug)]
pub enum WaveformError {
    Settings,
    TooManyBuckets,
    Window,
    Cancelled,
    Wav(WavError),
}
impl std::fmt::Display for WaveformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings => f.write_str("波形の基本区間は1〜65536サンプルフレームです"),
            Self::TooManyBuckets => f.write_str("波形の基本区間数が262144を超えます"),
            Self::Window => f.write_str("波形の取得範囲が不正です（1〜4096区間）"),
            Self::Cancelled => f.write_str("波形解析を取り消しました"),
            Self::Wav(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for WaveformError {}
impl From<WavError> for WaveformError {
    fn from(e: WavError) -> Self {
        Self::Wav(e)
    }
}
impl Waveform {
    pub(crate) fn allocated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.source_sha256.capacity()
            + (self.rms.capacity() + self.sample_peak.capacity()) * std::mem::size_of::<f64>()
            + self.levels.capacity() * std::mem::size_of::<WaveformLevel>()
            + self
                .levels
                .iter()
                .map(|level| {
                    level.channels.capacity() * std::mem::size_of::<Vec<SamplePeak>>()
                        + level
                            .channels
                            .iter()
                            .map(|peaks| peaks.capacity() * std::mem::size_of::<SamplePeak>())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }

    pub fn levels(&self) -> &[WaveformLevel] {
        &self.levels
    }
    /// Read only the requested channel/range. The final partial bucket keeps its exact end.
    pub fn window(
        &self,
        level: usize,
        channel: usize,
        start: usize,
        count: usize,
    ) -> Result<WaveformWindow<'_>, WaveformError> {
        let level = self.levels.get(level).ok_or(WaveformError::Window)?;
        let peaks = level.channels.get(channel).ok_or(WaveformError::Window)?;
        if count == 0 || count > MAX_WINDOW_BUCKETS || start > peaks.len() {
            return Err(WaveformError::Window);
        }
        let end = start.saturating_add(count).min(peaks.len());
        Ok(WaveformWindow {
            start_frame: (start as u64 * level.frames_per_bucket).min(self.source_frames),
            end_frame: (end as u64 * level.frames_per_bucket).min(self.source_frames),
            frames_per_bucket: level.frames_per_bucket,
            peaks: &peaks[start..end],
        })
    }
}
fn check(cancel: &AtomicBool) -> Result<(), WaveformError> {
    if cancel.load(Ordering::Acquire) {
        Err(WaveformError::Cancelled)
    } else {
        Ok(())
    }
}

/// No per-sample allocation, downmix, normalization or file writes. The caller caches using
/// source_sha256, version AND base_frames, and labels this as a source waveform.
pub fn analyze_waveform(
    wav: &VerifiedWav,
    base_frames: u32,
    cancel: &AtomicBool,
) -> Result<Waveform, WaveformError> {
    let info = wav.info();
    let mut reader = crate::wav_container::reader(wav.bytes())?;
    let scale = 2f64.powi(i32::from(info.bits_per_sample) - 1);
    let samples: Box<dyn Iterator<Item = Result<f64, hound::Error>> + '_> = if info.is_float {
        Box::new(reader.samples::<f32>().map(|s| s.map(f64::from)))
    } else {
        Box::new(
            reader
                .samples::<i32>()
                .map(move |s| s.map(|s| f64::from(s) / scale)),
        )
    };
    analyze_samples(info, samples, base_frames, cancel)
}

pub(crate) fn analyze_samples(
    info: &crate::WavInfo,
    mut samples: impl Iterator<Item = Result<f64, hound::Error>>,
    base_frames: u32,
    cancel: &AtomicBool,
) -> Result<Waveform, WaveformError> {
    check(cancel)?;
    if !(1..=65536).contains(&base_frames) {
        return Err(WaveformError::Settings);
    }
    let width = u64::from(base_frames);
    let count = info.frames.div_ceil(width);
    if count > MAX_BUCKETS {
        return Err(WaveformError::TooManyBuckets);
    }
    let n = usize::from(info.channels);
    let mut channels = vec![Vec::with_capacity(count as usize); n];
    let mut energy = vec![0f64; n];
    let mut sample_peak = vec![0f64; n];
    for frame in 0..info.frames {
        if frame % 4096 == 0 {
            check(cancel)?;
        }
        for channel in 0..n {
            let sample = samples
                .next()
                .ok_or(WavError::Container)?
                .map_err(|e| WavError::Decode(e.to_string()))?;
            if !sample.is_finite() {
                return Err(WavError::NonFiniteSample.into());
            }
            energy[channel] += sample * sample;
            sample_peak[channel] = sample_peak[channel].max(sample.abs());
            if frame % width == 0 {
                channels[channel].push(SamplePeak {
                    min: sample,
                    max: sample,
                });
            } else {
                let peak = channels[channel]
                    .last_mut()
                    .expect("first frame initialized bucket");
                peak.min = peak.min.min(sample);
                peak.max = peak.max.max(sample);
            }
        }
    }
    if samples.next().is_some() {
        return Err(WavError::Container.into());
    }
    let mut levels = vec![WaveformLevel {
        frames_per_bucket: width,
        channels,
    }];
    while levels.last().unwrap().bucket_count() > 1 {
        check(cancel)?;
        let previous = levels.last().unwrap();
        let mut channels = Vec::with_capacity(n);
        for peaks in &previous.channels {
            let mut reduced = Vec::with_capacity(peaks.len().div_ceil(2));
            for (index, pair) in peaks.chunks(2).enumerate() {
                if index % 4096 == 0 {
                    check(cancel)?;
                }
                let mut peak = pair[0];
                if let Some(next) = pair.get(1) {
                    peak.min = peak.min.min(next.min);
                    peak.max = peak.max.max(next.max);
                }
                reduced.push(peak);
            }
            channels.push(reduced);
        }
        levels.push(WaveformLevel {
            frames_per_bucket: previous.frames_per_bucket * 2,
            channels,
        });
    }
    check(cancel)?;
    Ok(Waveform {
        version: WAVEFORM_VERSION,
        source_sha256: info.content_hash.clone(),
        source_frames: info.frames,
        sample_rate: info.sample_rate,
        channels: info.channels,
        rms: energy
            .into_iter()
            .map(|e| (e / info.frames as f64).sqrt())
            .collect(),
        sample_peak,
        levels,
    })
}
