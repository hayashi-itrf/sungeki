//! Deterministic amplitude-based mouth timing. These are not phoneme labels.
use crate::{VerifiedWav, WavError};
use std::sync::atomic::{AtomicBool, Ordering};

use sungeki_project::MAX_LIPSYNC_WINDOWS;
pub use sungeki_project::{
    LIPSYNC_ANALYSIS_VERSION, LipSyncAnalysis, LipSyncSettings, MouthInterval,
};

#[derive(Debug)]
pub enum LipSyncError {
    Settings,
    TooLong,
    Cancelled,
    Wav(WavError),
}
impl std::fmt::Display for LipSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings => f.write_str("口パク解析の設定値が範囲外です"),
            Self::TooLong => f.write_str("口パク解析は100,000区間までです"),
            Self::Cancelled => f.write_str("口パク解析を取り消しました"),
            Self::Wav(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for LipSyncError {}
impl From<WavError> for LipSyncError {
    fn from(value: WavError) -> Self {
        Self::Wav(value)
    }
}
fn cancelled(flag: &AtomicBool) -> Result<(), LipSyncError> {
    if flag.load(Ordering::Acquire) {
        Err(LipSyncError::Cancelled)
    } else {
        Ok(())
    }
}

/// Uses all channels' squared samples, avoiding phase cancellation. Processes
/// owned, verified originals only; bounded window storage, no per-sample vector.
/// Does not mutate Project, normalize audio, or start an engine.
pub fn analyze_lipsync(
    wav: &VerifiedWav,
    settings: LipSyncSettings,
    cancel: &AtomicBool,
) -> Result<LipSyncAnalysis, LipSyncError> {
    cancelled(cancel)?;
    settings.validate().map_err(|_| LipSyncError::Settings)?;
    let info = wav.info();
    let rate = u64::from(info.sample_rate);
    let period = rate * u64::from(settings.window_us);
    let count = (info.frames * 1_000_000).div_ceil(period);
    if count > MAX_LIPSYNC_WINDOWS {
        return Err(LipSyncError::TooLong);
    }
    let mut reader = crate::wav_container::reader(wav.bytes())?;
    let spec = reader.spec();
    let scale = 2f64.powi(i32::from(spec.bits_per_sample) - 1);
    let mut samples: Box<dyn Iterator<Item = Result<f64, hound::Error>> + '_> = if info.is_float {
        Box::new(reader.samples::<f32>().map(|v| v.map(f64::from)))
    } else {
        Box::new(
            reader
                .samples::<i32>()
                .map(move |v| v.map(|v| f64::from(v) / scale)),
        )
    };
    let mut windows = Vec::with_capacity(count as usize);
    let mut start = 0;
    let mut peak = 0f64;
    for index in 1..=count {
        cancelled(cancel)?;
        // Derive every boundary independently; no accumulating rounded windows.
        let end = (index * period).div_ceil(1_000_000).min(info.frames);
        if end == start {
            continue;
        }
        let mut energy = 0f64;
        for frame in start..end {
            if frame % 4096 == 0 {
                cancelled(cancel)?;
            }
            for _ in 0..spec.channels {
                let sample = samples
                    .next()
                    .ok_or(WavError::Container)?
                    .map_err(|e| WavError::Decode(e.to_string()))?;
                energy += sample * sample;
            }
        }
        let rms = (energy / ((end - start) * u64::from(spec.channels)) as f64).sqrt();
        peak = peak.max(rms);
        windows.push((
            MouthInterval {
                start_frame: start,
                end_frame: end,
            },
            rms,
        ));
        start = end;
    }
    if start != info.frames || samples.next().is_some() {
        return Err(WavError::Container.into());
    }
    let threshold = (f64::from(settings.floor_ppm) / 1_000_000.)
        .max(peak * f64::from(settings.peak_ratio_ppm) / 1_000_000.);
    let mut runs: Vec<MouthInterval> = vec![];
    for (span, rms) in windows {
        if rms < threshold {
            continue;
        }
        if let Some(last) = runs.last_mut().filter(|r| r.end_frame == span.start_frame) {
            last.end_frame = span.end_frame;
        } else {
            runs.push(span);
        }
    }
    // Reject isolated short bursts before filling small internal gaps. Never
    // extend speech into leading/trailing silence to meet minimum durations.
    runs.retain(|s| {
        (s.end_frame - s.start_frame) * 1_000_000 >= rate * u64::from(settings.min_open_us)
    });
    let mut open: Vec<MouthInterval> = vec![];
    for span in runs {
        if let Some(last) = open.last_mut().filter(|r| {
            (span.start_frame - r.end_frame) * 1_000_000 < rate * u64::from(settings.min_close_us)
        }) {
            last.end_frame = span.end_frame;
        } else {
            open.push(span);
        }
    }
    cancelled(cancel)?;
    Ok(LipSyncAnalysis {
        version: LIPSYNC_ANALYSIS_VERSION,
        source_sha256: info.content_hash.clone(),
        settings,
        sample_rate: info.sample_rate,
        source_frames: info.frames,
        peak_rms: peak,
        threshold_rms: threshold,
        open,
    })
}
