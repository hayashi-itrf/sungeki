//! Read-only movie inspection and original-frame preparation.
//! Metadata inspection alone does not guarantee decoding or proxy timing.
mod capture;
pub mod display;
pub mod frames;
mod metadata;
pub mod rendered_frame;
use crate::{ProcessSpec, StopReason, run_process};
pub use metadata::{AudioStream, Rational, StreamClock, VideoMetadata, VideoStream};
use std::{
    ffi::OsString,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum ProbeError {
    Io(std::io::Error),
    Invalid(&'static str),
    Changed,
    Stopped(StopReason),
    Worker(String),
}
impl From<std::io::Error> for ProbeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Invalid(s) => write!(f, "動画の検査条件・情報が不正です: {s}"),
            Self::Changed => f.write_str("動画原本が検査中に変更されました"),
            Self::Stopped(s) => write!(f, "動画検査を停止しました: {s:?}"),
            Self::Worker(s) => write!(f, "動画検査に失敗しました: {s}"),
        }
    }
}
impl std::error::Error for ProbeError {}
pub struct ProbeRuntime<'a> {
    pub ffprobe: &'a Path,
    /// Existing configured SSD directory. Inspection snapshots are removed on
    /// return; PreparedVideo retains its snapshot until dropped.
    pub root: &'a Path,
    pub timeout: Duration,
    pub max_source_bytes: u64,
}
pub use sungeki_project::VideoProbe as ProbedVideo;
fn check(cancel: &AtomicBool, deadline: Instant) -> Result<(), ProbeError> {
    if cancel.load(Ordering::Acquire) {
        return Err(ProbeError::Stopped(StopReason::Cancelled));
    }
    if Instant::now() >= deadline {
        return Err(ProbeError::Stopped(StopReason::Deadline));
    }
    Ok(())
}
/// Blocking. Call outside the editor lock. Hash and inspect the same private
/// snapshot, never infer CFR from average/rate equality or substitute a proxy.
pub fn probe(
    path: &Path,
    runtime: &ProbeRuntime<'_>,
    cancel: &AtomicBool,
) -> Result<ProbedVideo, ProbeError> {
    validate_runtime(path, runtime)?;
    let deadline = Instant::now() + runtime.timeout;
    check(cancel, deadline)?;
    let captured = capture::Snapshot::new(path, runtime, || check(cancel, deadline))?;
    inspect_snapshot(&captured, runtime, cancel, deadline)
}
fn validate_runtime(path: &Path, runtime: &ProbeRuntime<'_>) -> Result<(), ProbeError> {
    if !path.is_absolute()
        || !runtime.root.is_absolute()
        || !runtime.ffprobe.is_absolute()
        || runtime.timeout.is_zero()
        || runtime.timeout > Duration::from_secs(300)
        || runtime.max_source_bytes == 0
        || runtime.max_source_bytes > 4 * 1024 * 1024 * 1024
    {
        return Err(ProbeError::Invalid("path/limits"));
    }
    Ok(())
}
fn inspect_snapshot(
    captured: &capture::Snapshot,
    runtime: &ProbeRuntime<'_>,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<ProbedVideo, ProbeError> {
    let args: Vec<OsString> = [
        "-v",
        "error",
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        "mov,matroska,webm",
        "-show_streams",
        "-show_format",
        "-of",
        "json",
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .chain([captured.path.as_os_str().to_owned()])
    .collect();
    check(cancel, deadline)?;
    let output = run_process(
        &ProcessSpec {
            program: runtime.ffprobe,
            args: &args,
            directory: &captured.root,
            timeout: deadline.saturating_duration_since(Instant::now()),
            max_output_bytes: 1024 * 1024,
        },
        cancel,
    )?;
    if let Some(reason) = output.stopped {
        return Err(ProbeError::Stopped(reason));
    }
    if !output.status.success() {
        return Err(ProbeError::Worker(
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2048)
                .collect(),
        ));
    }
    check(cancel, deadline)?;
    let metadata = metadata::parse(&output.stdout)?;
    let probe = ProbedVideo {
        sha256: captured.sha256.clone(),
        byte_length: captured.byte_length,
        metadata,
    };
    probe
        .validate()
        .map_err(|_| ProbeError::Invalid("source metadata bounds"))?;
    Ok(probe)
}
