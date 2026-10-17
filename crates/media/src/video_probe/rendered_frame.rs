//! Extract a frame from a verified completed movie, not from the current Project.
//! Blocking correctness baseline; snapshot and decoder never run under editor locks.
use super::{ProbeError, ProbeRuntime, capture::Snapshot, check, validate_runtime};
use crate::{ProcessSpec, run_process};
use std::{ffi::OsString, path::Path, sync::atomic::AtomicBool, time::Instant};

pub struct FrameRequest<'a> {
    pub source: &'a Path,
    pub sha256: &'a str,
    pub byte_length: u64,
    pub frame_index: u64,
    pub frame_count: u64,
    /// Output dimensions are bounded by the caller's preserved aspect ratio.
    pub width: u32,
    pub height: u32,
}

/// Decode presentation frame `frame_index` from the beginning. Deliberately
/// avoid an approximate timestamp/keyframe seek that could return a neighbor.
/// All temporary source bytes are removed on success, cancellation or failure.
pub fn png(
    request: &FrameRequest<'_>,
    ffmpeg: &Path,
    runtime: &ProbeRuntime<'_>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ProbeError> {
    validate_runtime(request.source, runtime)?;
    if !ffmpeg.is_absolute()
        || request.frame_index >= request.frame_count
        || request.byte_length == 0
        || request.byte_length > runtime.max_source_bytes
        || request.sha256.len() != 64
        || !request.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        || request.width == 0
        || request.height == 0
        || request.width > 960
        || request.height > 960
    {
        return Err(ProbeError::Invalid("rendered frame request"));
    }
    let deadline = Instant::now() + runtime.timeout;
    let snapshot = Snapshot::new(request.source, runtime, || check(cancel, deadline))?;
    if snapshot.sha256 != request.sha256 || snapshot.byte_length != request.byte_length {
        return Err(ProbeError::Changed);
    }
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        "mov",
        "-i",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    args.push(snapshot.path.as_os_str().to_owned());
    args.extend([
        "-map".into(),
        "0:v:0".into(),
        "-vf".into(),
        format!(
            "select=eq(n\\,{}),scale={}:{}",
            request.frame_index, request.width, request.height
        )
        .into(),
    ]);
    args.extend(
        [
            "-frames:v",
            "1",
            "-an",
            "-sn",
            "-dn",
            "-fps_mode",
            "passthrough",
            "-threads",
            "1",
            "-c:v",
            "png",
            "-f",
            "image2pipe",
            "pipe:1",
        ]
        .into_iter()
        .map(Into::into),
    );
    check(cancel, deadline)?;
    let output = run_process(
        &ProcessSpec {
            program: ffmpeg,
            args: &args,
            directory: &snapshot.root,
            timeout: deadline.saturating_duration_since(Instant::now()),
            max_output_bytes: 4 * 1024 * 1024,
        },
        cancel,
    )?;
    if let Some(reason) = output.stopped {
        return Err(ProbeError::Stopped(reason));
    }
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(ProbeError::Worker(
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2048)
                .collect(),
        ));
    }
    check(cancel, deadline)?;
    let bytes = output.stdout;
    if bytes.len() < 33
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[8..16] != b"\0\0\0\rIHDR"
        || bytes[16..20] != request.width.to_be_bytes()
        || bytes[20..24] != request.height.to_be_bytes()
    {
        return Err(ProbeError::Invalid("missing or mismatched rendered PNG"));
    }
    Ok(bytes)
}
