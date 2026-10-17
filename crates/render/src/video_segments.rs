//! Bound animated image sequences to 120 links. Long static holds remain one
//! looped PNG input. Only private job files are linked; originals are untouched.
use crate::{
    RenderError,
    execute::{Runner, strings},
    originals::cancelled,
    plan::error,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use sungeki_project::FrameRate;

const BATCH_FRAMES: u64 = 120;
pub(crate) struct VideoSegments<'a> {
    fps: &'a FrameRate,
    pending: Vec<PathBuf>,
    first: u64,
    next: u64,
    segments: usize,
    concat: String,
}
impl<'a> VideoSegments<'a> {
    pub fn new(fps: &'a FrameRate) -> Self {
        Self {
            fps,
            pending: Vec::new(),
            first: 0,
            next: 0,
            segments: 0,
            concat: "ffconcat version 1.0\n".into(),
        }
    }
    pub fn push(
        &mut self,
        runner: &Runner<'_>,
        png: &Path,
        first: u64,
        end: u64,
    ) -> Result<(), RenderError> {
        cancelled(runner.flag)?;
        if first != self.next || end <= first {
            return Err(error("映像区間の順序または長さが不正です"));
        }
        let count = end - first;
        if count > BATCH_FRAMES {
            self.flush(runner)?;
            self.encode(runner, png, false, first, end)?;
        } else {
            if self.pending.len() as u64 + count > BATCH_FRAMES {
                self.flush(runner)?;
            }
            if self.pending.is_empty() {
                self.first = first;
            }
            self.pending.extend((0..count).map(|_| png.to_owned()));
        }
        self.next = end;
        Ok(())
    }
    fn flush(&mut self, runner: &Runner<'_>) -> Result<(), RenderError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let dir = runner.directory.join(format!("sequence-{}", self.segments));
        fs::create_dir(&dir)?;
        for (i, png) in self.pending.iter().enumerate() {
            cancelled(runner.flag)?;
            fs::hard_link(png, dir.join(format!("{i:03}.png")))?;
        }
        self.encode(
            runner,
            &dir.join("%03d.png"),
            true,
            self.first,
            self.first + self.pending.len() as u64,
        )?;
        // Never remove source PNGs; links are expendable after successful encode.
        fs::remove_dir_all(dir)?;
        self.pending.clear();
        Ok(())
    }
    fn encode(
        &mut self,
        runner: &Runner<'_>,
        input: &Path,
        sequence: bool,
        first: u64,
        end: u64,
    ) -> Result<(), RenderError> {
        let rate = format!("{}/{}", self.fps.numerator, self.fps.denominator);
        let mut args = if sequence {
            strings(&["-start_number", "0"])
        } else {
            strings(&["-loop", "1"])
        };
        let segment = format!("segment-{}.mp4", self.segments);
        args.extend(strings(&[
            "-framerate",
            &rate,
            "-i",
            input.to_str().ok_or_else(|| error("PNGパスが不正です"))?,
            "-frames:v",
            &(end - first).to_string(),
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-bf",
            "0",
            "-video_track_timescale",
            &self.fps.numerator.to_string(),
            &segment,
        ]));
        runner.ffmpeg(args)?;
        let boundary = |frame: u64| {
            (u128::from(frame) * 1_000_000 * u128::from(self.fps.denominator)
                + u128::from(self.fps.numerator) / 2)
                / u128::from(self.fps.numerator)
        };
        let us = boundary(end) - boundary(first);
        self.concat.push_str(&format!(
            "file '{segment}'\nduration {}.{:06}\n",
            us / 1_000_000,
            us % 1_000_000
        ));
        self.segments += 1;
        Ok(())
    }
    pub fn finish(mut self, runner: &Runner<'_>, frames: u64) -> Result<String, RenderError> {
        if self.next != frames {
            return Err(error("映像区間が予定の尺と一致しません"));
        }
        self.flush(runner)?;
        Ok(self.concat)
    }
}
