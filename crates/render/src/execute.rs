use crate::{
    RenderError, RenderPlan,
    originals::{cancelled, write_new},
    plan::error,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use sungeki_media::{ProcessSpec, run_process};

/// Work completed within a stage, not an estimate of total elapsed time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderProgress {
    Preparing,
    Drawing { completed: u64, total: u64 },
    Audio,
    Encoding,
    Verifying,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderRuntime {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrm: Option<crate::vrm_worker::VrmRuntime>,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub font: PathBuf,
    pub font_sha256: String,
    pub deadline: Duration,
}
#[derive(Debug, Serialize)]
pub struct RenderArtifact {
    /// A verified staging artifact, not yet published or journaled by the host.
    pub path: PathBuf,
    pub sha256: String,
    pub byte_length: u64,
    pub frame_count: u64,
    pub font_sha256: String,
    pub layout_version: u32,
    pub probe: Value,
    pub frame_cache: crate::FrameCacheStats,
}
pub(crate) struct Runner<'a> {
    pub runtime: crate::execution_runtime::ExecutionRuntime<'a>,
    pub directory: &'a Path,
    pub flag: &'a AtomicBool,
    pub end: Instant,
}
impl Runner<'_> {
    pub fn raster_error(&self, failure: impl ToString) -> RenderError {
        if self.flag.load(std::sync::atomic::Ordering::SeqCst) {
            RenderError::Cancelled
        } else {
            // Native rasterization has returned; no child process can still use
            // its intermediates, even when the input/font was invalid.
            RenderError::Settled(failure.to_string())
        }
    }
    pub fn check_active(&self) -> Result<(), RenderError> {
        cancelled(self.flag)?;
        if Instant::now() >= self.end {
            return Err(error("描画の実行期限を超えました"));
        }
        Ok(())
    }
    pub fn run(&self, program: &Path, args: Vec<OsString>) -> Result<Vec<u8>, RenderError> {
        cancelled(self.flag)?;
        let remaining = self
            .end
            .checked_duration_since(Instant::now())
            .filter(|v| !v.is_zero())
            .ok_or_else(|| error("描画の実行期限を超えました"))?;
        let result = run_process(
            &ProcessSpec {
                program,
                args: &args,
                directory: self.directory,
                timeout: remaining,
                max_output_bytes: 1024 * 1024,
            },
            self.flag,
        )?;
        if result.stopped == Some(sungeki_media::StopReason::Cancelled) {
            return Err(RenderError::Cancelled);
        }
        if !result.succeeded() {
            return Err(RenderError::Settled(format!(
                "外部描画処理に失敗しました: {:?}: {}",
                result.stopped,
                String::from_utf8_lossy(&result.stderr)
            )));
        }
        Ok(result.stdout)
    }
    pub fn ffmpeg(&self, args: Vec<OsString>) -> Result<(), RenderError> {
        let mut base = strings(&["-nostdin", "-v", "error", "-xerror", "-n"]);
        base.extend(args);
        self.run(self.runtime.ffmpeg(), base).map(|_| ())
    }
}
pub(crate) fn strings(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

/// Caller reserves a NEW private directory; this function never publishes a
/// user destination. On error the host retains/cleans its own staging directory.
/// The same snapshot must already have passed synthesis fingerprint checks.
pub fn render_staged(
    plan: &RenderPlan,
    runtime: &RenderRuntime,
    directory: &Path,
    flag: &AtomicBool,
    progress: &mut dyn FnMut(RenderProgress),
) -> Result<RenderArtifact, RenderError> {
    cancelled(flag)?;
    progress(RenderProgress::Preparing);
    let runner = crate::prepare::runner(runtime, directory, flag)?;
    // Audio-only estimate plus bounded originals; video size depends on content.
    let pcm = (plan.audio_samples() + 1024) * 8 + 65536;
    sungeki_media::ensure_available_space(
        directory,
        pcm * if plan.scene.audio_mix.normalization.is_some() {
            4
        } else {
            1
        } + 1024 * 1024 * 1024
            + 64 * 1024 * 1024,
    )?;
    let mut visuals = crate::prepare::visuals(plan, &runner)?;
    progress(RenderProgress::Drawing {
        completed: 0,
        total: plan.frame_count(),
    });
    let fps = &plan.scene.canvas.fps;
    let mut segments = crate::video_segments::VideoSegments::new(fps);
    let mut cache = crate::frame_cache::FrameCache::batched();
    let mut pending = Vec::new();
    for (i, span) in plan.scene.visual_spans().enumerate() {
        cancelled(flag)?;
        let layers: Vec<_> = span
            .layer_indices
            .iter()
            .map(|index| &plan.scene.layers[*index])
            .collect();
        let png = visuals.frame(
            &mut cache,
            &runner,
            &plan.scene.canvas,
            &layers,
            i,
            span.frames.first,
        )?;
        pending.push((png, span.frames.first, span.frames.end));
        if pending.len() == 32 {
            cache.flush(&runner, &plan.scene.canvas)?;
            for (png, first, end) in pending.drain(..) {
                segments.push(&runner, &png, first, end)?;
            }
            progress(RenderProgress::Drawing {
                completed: span.frames.end,
                total: plan.frame_count(),
            });
        }
    }
    cache.flush(&runner, &plan.scene.canvas)?;
    for (png, first, end) in pending {
        segments.push(&runner, &png, first, end)?;
    }
    let concat = segments.finish(&runner, plan.frame_count())?;
    write_new(&directory.join("video.ffconcat"), concat.as_bytes())?;
    progress(RenderProgress::Audio);
    let mix_hash = crate::audio_mix::freeze(plan, &runner, &visuals.resources)?;
    let master = crate::master_audio::prepare(plan, &runner, &mix_hash)?;
    let mut args = strings(&[
        "-f",
        "concat",
        "-safe",
        "1",
        "-i",
        "video.ffconcat",
        "-i",
        if master.is_some() {
            "master.wav"
        } else {
            "mix.wav"
        },
    ]);
    if !plan.chapters.is_empty() {
        write_new(
            &directory.join("chapters.ffmetadata"),
            crate::chapters::metadata(&plan.chapters).as_bytes(),
        )?;
        args.extend(strings(&["-f", "ffmetadata", "-i", "chapters.ffmetadata"]));
    }
    // Names are individual argv values, never shell text or FFMETADATA lines.
    // FFmpeg's metadata reader treats even an escaped terminal backslash as
    // line continuation. Keeping names out of that parser preserves them.
    for (index, chapter) in plan.chapters.iter().enumerate() {
        args.extend(strings(&[
            &format!("-metadata:c:{index}"),
            &format!("title={}", chapter.title),
        ]));
    }
    args.extend(strings(&[
        "-map",
        "0:v:0",
        "-map",
        "1:a:0",
        "-c:v",
        "copy",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-map_metadata",
        if plan.chapters.is_empty() { "-1" } else { "2" },
        "-map_chapters",
        if plan.chapters.is_empty() { "-1" } else { "2" },
        "-video_track_timescale",
        &fps.numerator.to_string(),
        "-movflags",
        "+faststart",
        "output.mp4",
    ]));
    progress(RenderProgress::Encoding);
    runner.ffmpeg(args)?;
    progress(RenderProgress::Verifying);
    let mut probe = crate::verify::inspect(plan, &runner)?;
    if let Some(report) = master {
        probe["master_audio"] = crate::master_audio::verify_encoded(plan, &runner, report)?;
    }
    cancelled(flag)?;
    let path = directory.join("output.mp4");
    fs::File::open(&path)?.sync_all()?;
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    use std::io::Read;
    let mut source = fs::File::open(&path)?;
    let mut buffer = [0u8; 65536];
    loop {
        cancelled(flag)?;
        if Instant::now() >= runner.end {
            return Err(error("出力検証の実行期限を超えました"));
        }
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(RenderArtifact {
        byte_length: fs::metadata(&path)?.len(),
        path,
        sha256: format!("{:x}", digest.finalize()),
        frame_count: plan.frame_count(),
        font_sha256: runtime.font_sha256.clone(),
        layout_version: crate::CURRENT_LAYOUT_VERSION,
        probe,
        frame_cache: cache.stats,
    })
}
