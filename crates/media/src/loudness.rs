//! Read-only loudness measurement of immutable, validated source WAV bytes.
use crate::{ProcessSpec, StopReason, VerifiedWav, run_process};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const LOUDNESS_VERSION: u32 = 2;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum LoudnessValue {
    Measured {
        integrated_lufs: f64,
        true_peak_db: f64,
        range_lu: f64,
        threshold_lufs: f64,
    },
    NotMeasurable {
        reason: LoudnessUnavailable,
        true_peak_db: Option<f64>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoudnessUnavailable {
    TooShort,
    BelowGateOrSilence,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoudnessAnalysis {
    pub version: u32,
    pub method: String,
    pub source_sha256: String,
    pub source_frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub source_pre_gain: bool,
    pub dual_mono: bool,
    pub measurement: LoudnessValue,
}
#[derive(Debug)]
pub enum LoudnessError {
    InvalidRuntime,
    Cancelled,
    Stopped(StopReason),
    Io(std::io::Error),
    Worker(String),
    InvalidReport,
}
impl std::fmt::Display for LoudnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "音量測定に失敗しました: {self:?}")
    }
}
impl std::error::Error for LoudnessError {}
impl From<std::io::Error> for LoudnessError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub(crate) struct Work(pub(crate) PathBuf);
impl Drop for Work {
    fn drop(&mut self) {
        // Only our private capture is removed, never a source path or caller's directory.
        let _ = fs::remove_file(self.0.join("source.wav"));
        let _ = fs::remove_dir(&self.0);
    }
}
pub(crate) fn check(flag: &AtomicBool) -> Result<(), LoudnessError> {
    if flag.load(Ordering::Acquire) {
        Err(LoudnessError::Cancelled)
    } else {
        Ok(())
    }
}
/// Caller owns a private existing work root (SSD in the host runtime). Invoke on
/// a blocking worker without the editor lock. No gain is applied to the source.
/// Shorter than 400ms is explicitly unmeasurable under this analysis policy.
pub fn measure_loudness(
    audio: &VerifiedWav,
    ffmpeg: &Path,
    work_root: &Path,
    timeout: Duration,
    flag: &AtomicBool,
) -> Result<LoudnessAnalysis, LoudnessError> {
    check(flag)?;
    if !ffmpeg.is_absolute()
        || !work_root.is_absolute()
        || timeout.is_zero()
        || timeout > Duration::from_secs(3600)
        || !fs::symlink_metadata(work_root)?.is_dir()
    {
        return Err(LoudnessError::InvalidRuntime);
    }
    let info = audio.info();
    let measurement = if info.frames * 1_000_000 < u64::from(info.sample_rate) * 400_000 {
        LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::TooShort,
            true_peak_db: None,
        }
    } else {
        run_measurement(audio, ffmpeg, work_root, timeout, flag)?
    };
    check(flag)?;
    Ok(LoudnessAnalysis {
        version: LOUDNESS_VERSION,
        method: "ffmpeg_ebur128_summary_192k".into(),
        source_sha256: info.content_hash.clone(),
        source_frames: info.frames,
        sample_rate: info.sample_rate,
        channels: info.channels,
        source_pre_gain: true,
        dual_mono: false,
        measurement,
    })
}

fn capture(audio: &VerifiedWav, root: &Path, flag: &AtomicBool) -> Result<Work, LoudnessError> {
    let directory = root.join(format!("loudness-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let work = Work(directory);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(work.0.join("source.wav"))?;
    for chunk in audio.bytes().chunks(65536) {
        check(flag)?;
        file.write_all(chunk)?;
    }
    Ok(work)
}
fn run_measurement(
    audio: &VerifiedWav,
    ffmpeg: &Path,
    root: &Path,
    timeout: Duration,
    flag: &AtomicBool,
) -> Result<LoudnessValue, LoudnessError> {
    let start = Instant::now();
    let work = capture(audio, root, flag)?;
    let remaining = timeout
        .checked_sub(start.elapsed())
        .filter(|d| !d.is_zero())
        .ok_or(LoudnessError::Stopped(StopReason::Deadline))?;
    let report = run_captured(&work, ffmpeg, remaining, flag)?;
    work.finish()?;
    Ok(report)
}
impl Work {
    pub(crate) fn finish(self) -> Result<(), LoudnessError> {
        fs::remove_file(self.0.join("source.wav"))?;
        fs::remove_dir(&self.0)?;
        Ok(())
    }
}
pub(crate) fn run_captured(
    work: &Work,
    ffmpeg: &Path,
    remaining: Duration,
    flag: &AtomicBool,
) -> Result<LoudnessValue, LoudnessError> {
    let args: Vec<OsString> = [
        "-nostdin",
        "-hide_banner",
        "-v",
        "info",
        "-nostats",
        "-xerror",
        "-i",
        "source.wav",
        "-map",
        "0:a:0",
        "-af",
        "aresample=192000,ebur128=peak=true:dualmono=false:framelog=verbose",
        "-f",
        "null",
        "-",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let outcome = run_process(
        &ProcessSpec {
            program: ffmpeg,
            args: &args,
            directory: &work.0,
            timeout: remaining,
            max_output_bytes: 65536,
        },
        flag,
    );
    check(flag)?;
    let output = outcome?;
    if let Some(reason) = output.stopped {
        return Err(LoudnessError::Stopped(reason));
    }
    if !output.status.success() {
        return Err(LoudnessError::Worker(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    crate::loudness_report::parse(&output.stderr)
}
