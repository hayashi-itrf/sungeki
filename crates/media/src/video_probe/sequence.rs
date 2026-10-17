//! Bounded, one-process frame delivery. Callback results remain tentative until
//! the entire operation succeeds; renderers must not publish partial output.
use super::{PreparedVideo, ProbeError, SourceFrame, check};
use crate::{ProcessSpec, run_process_to_writer};
use std::{
    io::{self, Write},
    ops::Range,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
impl PreparedVideo {
    /// Decode a contiguous presentation-index range using one worker. At most
    /// 4GiB raw output per call; long renders can request consecutive ranges.
    /// RAM is bounded to two raw frames plus caller-retained output. The callback
    /// must return promptly and must not publish output before success.
    pub fn decode_range(
        &self,
        range: Range<usize>,
        ffmpeg: &Path,
        timeout: Duration,
        cancel: &AtomicBool,
        mut receive: impl FnMut(SourceFrame) -> io::Result<()>,
    ) -> Result<(), ProbeError> {
        if !ffmpeg.is_absolute()
            || timeout.is_zero()
            || timeout > Duration::from_secs(300)
            || range.start >= range.end
            || range.end > self.frame_count()
        {
            return Err(ProbeError::Invalid("decode range/runtime"));
        }
        let deadline = Instant::now() + timeout;
        check(cancel, deadline)?;
        let stream = &self.probe.metadata.video;
        let size = u64::from(stream.width) * u64::from(stream.height) * 4;
        let count = (range.end - range.start) as u64;
        let total = size
            .checked_mul(count)
            .ok_or(ProbeError::Invalid("decode range size"))?;
        if size == 0 || size > 64 * 1024 * 1024 || total > 4 * 1024 * 1024 * 1024 {
            return Err(ProbeError::Invalid("decode range exceeds output limit"));
        }
        let args = self.decode_args(
            &format!("select=between(n\\,{}\\,{})", range.start, range.end - 1),
            count as usize,
        );
        let mut sink = Frames {
            buffer: Vec::with_capacity(size as usize),
            size: size as usize,
            next: range.start,
            end: range.end,
            receive: &mut receive,
            video: self,
            cancel,
            deadline,
        };
        let result = run_process_to_writer(
            &ProcessSpec {
                program: ffmpeg,
                args: &args,
                directory: &self.snapshot.root,
                timeout: deadline.saturating_duration_since(Instant::now()),
                max_output_bytes: 65536,
            },
            cancel,
            &mut sink,
            total as usize,
        );
        check(cancel, deadline)?;
        let result = result?;
        if let Some(stop) = result.stopped {
            return Err(ProbeError::Stopped(stop));
        }
        if !result.status.success() || !result.stderr.is_empty() {
            return Err(ProbeError::Worker(
                String::from_utf8_lossy(&result.stderr)
                    .chars()
                    .take(2048)
                    .collect(),
            ));
        }
        check(cancel, deadline)?;
        if sink.next != range.end || !sink.buffer.is_empty() {
            return Err(ProbeError::Invalid("incomplete decoded frame sequence"));
        }
        Ok(())
    }
}
struct Frames<'a, F> {
    buffer: Vec<u8>,
    size: usize,
    next: usize,
    end: usize,
    receive: &'a mut F,
    video: &'a PreparedVideo,
    cancel: &'a AtomicBool,
    deadline: Instant,
}
impl<F: FnMut(SourceFrame) -> io::Result<()>> Write for Frames<'_, F> {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let total = bytes.len();
        while !bytes.is_empty() {
            check(self.cancel, self.deadline).map_err(io::Error::other)?;
            if self.next == self.end {
                return Err(io::Error::other("extra decoded frame"));
            }
            let n = bytes.len().min(self.size - self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..n]);
            bytes = &bytes[n..];
            if self.buffer.len() == self.size {
                let rgba = std::mem::replace(&mut self.buffer, Vec::with_capacity(self.size));
                (self.receive)(SourceFrame {
                    rgba,
                    stream: self.video.probe.metadata.video.clone(),
                    frame_index: self.next,
                    pts: self.video.clock.pts[self.next],
                })
                .map_err(io::Error::other)?;
                self.next += 1;
            }
        }
        Ok(total)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
