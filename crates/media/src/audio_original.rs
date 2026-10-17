//! Capture an audio original without trusting its path or saved probe alone.
use crate::{MAX_WAV_BYTES, WaveformCacheError};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_project::AudioProbe;
#[path = "audio_source_scan.rs"]
mod source_scan;
pub use source_scan::source_identity;
pub(crate) fn cancelled(flag: &AtomicBool) -> Result<(), WaveformCacheError> {
    if flag.load(Ordering::Acquire) {
        Err(WaveformCacheError::Cancelled)
    } else {
        Ok(())
    }
}
pub fn capture(
    path: &Path,
    probe: &AudioProbe,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, WaveformCacheError> {
    capture_with_cancel(path, probe, || cancel.load(Ordering::Acquire))
}
pub fn capture_with_cancel(
    path: &Path,
    probe: &AudioProbe,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>, WaveformCacheError> {
    probe
        .validate()
        .map_err(|_| WaveformCacheError::InvalidProbe)?;
    collect(path, Some(probe), cancelled)
}
pub(crate) fn read(
    path: &Path,
    probe: Option<&AudioProbe>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, WaveformCacheError> {
    collect(path, probe, || cancel.load(Ordering::Acquire))
}
fn collect(
    path: &Path,
    probe: Option<&AudioProbe>,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>, WaveformCacheError> {
    let mut bytes = Vec::new();
    source_scan::scan(path, probe, MAX_WAV_BYTES as u64, cancelled, |chunk| {
        bytes.extend_from_slice(chunk)
    })?;
    Ok(bytes)
}

pub fn inspect(path: &Path, cancel: &AtomicBool) -> Result<AudioProbe, WaveformCacheError> {
    let wav =
        crate::wav::inspect_wav(read(path, None, cancel)?).map_err(WaveformCacheError::Wav)?;
    cancelled(cancel)?;
    let info = wav.info();
    let probe = AudioProbe {
        encoding: None,
        sha256: info.content_hash.clone(),
        duration_us: sungeki_project::TimeUs(info.duration_us),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        byte_length: wav.bytes().len() as u64,
    };
    probe
        .validate()
        .map_err(|_| WaveformCacheError::InvalidProbe)?;
    Ok(probe)
}
