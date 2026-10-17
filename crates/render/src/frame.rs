//! Formal PNG output. Uses the movie's scene evaluation, originals, font and
//! image worker, without synthesis or audio decoding. The host owns publication.
use crate::{RenderError, RenderPlan, RenderRuntime, originals::cancelled, plan::error};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};
use sungeki_project::{Project, TimeUs};

/// Version of newly produced visual artifacts. Stored older artifacts retain their version.
pub const CURRENT_LAYOUT_VERSION: u32 = 9;

#[derive(Debug)]
pub struct FramePlan {
    render: RenderPlan,
    frame_index: u64,
}
impl FramePlan {
    pub fn new(project: &Project, frame_index: u64) -> Result<Self, RenderError> {
        let mut scene = sungeki_scene::ScenePlan::new(project).map_err(error)?;
        if frame_index >= scene.frame_count {
            return Err(error("指定フレームが作品の範囲外です"));
        }
        // Project remains untouched. A still image does not claim that speech
        // is synthesized/fresh or that the final mix is ready to render.
        scene.audio.clear();
        scene.audio_mix.normalization = None;
        // Keep the original clock and layer order, but prepare originals and
        // expensive animated images only for the requested still.
        scene.layers = scene
            .frame(frame_index)
            .ok_or_else(|| error("指定フレームがありません"))?
            .into_iter()
            .cloned()
            .collect();
        let render = RenderPlan::from_scene(project, scene, |_| Ok(()))?;
        Ok(Self {
            render,
            frame_index,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct FrameArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub frame_index: u64,
    pub time_us: TimeUs,
    pub font_sha256: String,
    pub layout_version: u32,
}

/// A NEW empty private directory is required. No user output is overwritten.
pub fn render_frame_staged(
    plan: &FramePlan,
    runtime: &RenderRuntime,
    directory: &Path,
    flag: &AtomicBool,
) -> Result<FrameArtifact, RenderError> {
    let runner = crate::prepare::runner(runtime, directory, flag)?;
    sungeki_media::ensure_available_space(directory, 1024 * 1024 * 1024 + 64 * 1024 * 1024)?;
    let mut visuals = crate::prepare::visuals(&plan.render, &runner)?;
    let scene = &plan.render.scene;
    let layers = scene
        .frame(plan.frame_index)
        .ok_or_else(|| error("指定フレームがありません"))?;
    let path = visuals.frame(
        &mut crate::frame_cache::FrameCache::default(),
        &runner,
        &scene.canvas,
        &layers,
        0,
        plan.frame_index,
    )?;
    cancelled(flag)?;
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(error("描画PNGが通常ファイルではありません"));
    }
    let probe = sungeki_assets::probe_image(&path, sungeki_assets::ProbeLimits::default())
        .map_err(error)?;
    if probe.format != sungeki_project::ImageFormat::Png
        || probe.width != scene.canvas.width
        || probe.height != scene.canvas.height
    {
        return Err(error("描画PNGの形式・寸法が計画と一致しません"));
    }
    cancelled(flag)?;
    if Instant::now() >= runner.end {
        return Err(error("フレーム検証の実行期限を超えました"));
    }
    fs::File::open(&path)?.sync_all()?;
    Ok(FrameArtifact {
        path,
        sha256: probe.sha256,
        byte_length: probe.byte_length,
        width: probe.width,
        height: probe.height,
        frame_index: plan.frame_index,
        time_us: scene
            .canvas
            .fps
            .frame_time(plan.frame_index)
            .ok_or_else(|| error("フレーム時刻が不正です"))?,
        font_sha256: runtime.font_sha256.clone(),
        layout_version: CURRENT_LAYOUT_VERSION,
    })
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
