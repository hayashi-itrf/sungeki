//! Decode original pixels against an exact presentation timeline. Not a proxy.
#[path = "audio.rs"]
mod audio;
#[path = "frame_clock.rs"]
mod clock;
#[path = "proxy.rs"]
pub mod proxy;
#[path = "sequence.rs"]
mod sequence;
use super::{
    ProbeError, ProbeRuntime, ProbedVideo, capture::Snapshot, check, inspect_snapshot,
    validate_runtime,
};
use crate::{ProcessSpec, run_process};
pub use audio::VideoAudioFile;
use std::{
    ffi::OsString,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use sungeki_project::{TimeUs, VideoStreamInfo};
pub struct PreparedVideo {
    snapshot: Snapshot,
    probe: ProbedVideo,
    clock: clock::Clock,
}
/// Pixels in source raster coordinates: display matrix and sample aspect have
/// NOT been applied. The compositor must consume `stream` for display geometry.
#[derive(Clone)]
pub struct SourceFrame {
    pub rgba: Vec<u8>,
    pub stream: VideoStreamInfo,
    pub frame_index: usize,
    pub pts: i64,
}
impl PreparedVideo {
    /// Borrowed private snapshot path for lifetime-bound stability checks.
    pub fn snapshot_path(&self) -> &Path {
        &self.snapshot.path
    }

    pub fn open(
        path: &Path,
        expected: &ProbedVideo,
        runtime: &ProbeRuntime<'_>,
        cancel: &AtomicBool,
    ) -> Result<Self, ProbeError> {
        validate_runtime(path, runtime)?;
        expected
            .validate()
            .map_err(|_| ProbeError::Invalid("invalid saved probe"))?;
        let deadline = Instant::now() + runtime.timeout;
        let snapshot = Snapshot::new(path, runtime, || check(cancel, deadline))?;
        if snapshot.sha256 != expected.sha256 || snapshot.byte_length != expected.byte_length {
            return Err(ProbeError::Changed);
        }
        let probe = inspect_snapshot(&snapshot, runtime, cancel, deadline)?;
        if probe != *expected {
            return Err(ProbeError::Changed);
        }
        let args: Vec<OsString> = [
            "-v",
            "error",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm",
            "-select_streams",
        ]
        .into_iter()
        .map(Into::into)
        .chain([probe.metadata.video.index.to_string().into()])
        .chain(
            [
                "-show_frames",
                "-show_entries",
                "frame=pts,width,height",
                "-of",
                "json",
                "-i",
            ]
            .into_iter()
            .map(Into::into),
        )
        .chain([snapshot.path.as_os_str().to_owned()])
        .collect();
        let bytes = run(
            &snapshot,
            runtime.ffprobe,
            &args,
            deadline,
            32 * 1024 * 1024,
            cancel,
        )?;
        let clock = clock::Clock::parse(&bytes, &probe)?;
        Ok(Self {
            snapshot,
            probe,
            clock,
        })
    }
    pub fn probe(&self) -> &ProbedVideo {
        &self.probe
    }
    /// Complete verified presentation timestamps, in source stream time base.
    pub fn frame_pts(&self) -> &[i64] {
        &self.clock.pts
    }
    pub fn frame_count(&self) -> usize {
        self.clock.pts.len()
    }
    /// Exact presentation-frame identity without decoding pixels.
    pub fn frame_index_at(&self, offset: TimeUs) -> Result<Option<usize>, ProbeError> {
        self.clock.index(offset)
    }
    /// Correctness baseline: decodes from the beginning for each request.
    /// A sequential/cache layer is still required for production rendering.
    pub fn decode_at(
        &self,
        offset: TimeUs,
        ffmpeg: &Path,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Option<SourceFrame>, ProbeError> {
        if !ffmpeg.is_absolute() || timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err(ProbeError::Invalid("decode runtime"));
        }
        let deadline = Instant::now() + timeout;
        check(cancel, deadline)?;
        let Some(index) = self.clock.index(offset)? else {
            return Ok(None);
        };
        let stream = &self.probe.metadata.video;
        let size = u64::from(stream.width) * u64::from(stream.height) * 4;
        if size > crate::MAX_WAV_BYTES as u64 {
            return Err(ProbeError::Invalid("decoded frame exceeds 64MiB"));
        }
        let args = self.decode_args(&format!("select=eq(n\\,{index})"), 1);
        let rgba = run(
            &self.snapshot,
            ffmpeg,
            &args,
            deadline,
            size as usize,
            cancel,
        )?;
        if rgba.len() != size as usize {
            return Err(ProbeError::Invalid("decoded frame size mismatch"));
        }
        Ok(Some(SourceFrame {
            rgba,
            stream: stream.clone(),
            frame_index: index,
            pts: self.clock.pts[index],
        }))
    }
    fn decode_args(&self, filter: &str, count: usize) -> Vec<OsString> {
        [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm",
            "-noautorotate",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .chain([self.snapshot.path.as_os_str().to_owned()])
        .chain([
            "-map".into(),
            format!("0:{}", self.probe.metadata.video.index).into(),
            "-vf".into(),
            filter.into(),
            "-frames:v".into(),
            count.to_string().into(),
        ])
        .chain(
            [
                "-an",
                "-sn",
                "-dn",
                "-fps_mode",
                "passthrough",
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "pipe:1",
            ]
            .into_iter()
            .map(Into::into),
        )
        .collect()
    }
}
fn run(
    snapshot: &Snapshot,
    program: &Path,
    args: &[OsString],
    deadline: Instant,
    limit: usize,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ProbeError> {
    check(cancel, deadline)?;
    let output = run_process(
        &ProcessSpec {
            program,
            args,
            directory: &snapshot.root,
            timeout: deadline.saturating_duration_since(Instant::now()),
            max_output_bytes: limit,
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
    Ok(output.stdout)
}
