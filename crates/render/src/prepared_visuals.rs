#[path = "prepared_vrm.rs"]
mod vrm;
use crate::{
    RenderError, RenderPlan, VideoLayerResolver, execute::Runner, frame_cache::FrameCache,
    originals::write_new, plan::error,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use sungeki_media::video_probe::{ProbeError, ProbeRuntime, frames::PreparedVideo};
use sungeki_project::{AssetLocator, AssetProbe, Canvas};
use sungeki_scene::{Layer, Primitive};
pub(crate) struct PreparedVisuals {
    pub resources: BTreeMap<String, Value>,
    resolver: VideoLayerResolver,
    vrm: vrm::PreparedVrm,
}
fn remaining(runner: &Runner<'_>) -> Result<Duration, RenderError> {
    runner
        .end
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .map(|d| d.min(Duration::from_secs(300)))
        .ok_or_else(|| error("動画素材の準備期限を超えました"))
}
impl PreparedVisuals {
    pub fn new(
        plan: &RenderPlan,
        runner: &Runner<'_>,
        mut resources: BTreeMap<String, Value>,
    ) -> Result<Self, RenderError> {
        let vrm = vrm::PreparedVrm::new(plan, runner, &mut resources)?;
        let mut videos = BTreeMap::new();
        for (id, asset) in &plan.sources {
            let AssetProbe::Video(probe) = &asset.probe else {
                continue;
            };
            let AssetLocator::Absolute { path } = &asset.locator else {
                return Err(error("動画原本を絶対パスへ解決してください"));
            };
            let video = PreparedVideo::open(
                Path::new(path),
                probe,
                &ProbeRuntime {
                    ffprobe: runner.runtime.ffprobe(),
                    root: runner.directory,
                    timeout: remaining(runner)?,
                    max_source_bytes: 4 * 1024 * 1024 * 1024,
                },
                runner.flag,
            )
            .map_err(|cause| {
                let message = format!(
                    "動画素材「{}」の原本を準備できません。保存場所と内容を確認してください。素材ID: {}、原本: {}。詳細: {}",
                    asset.name, id, path, cause
                );
                match cause {
                    ProbeError::Changed => RenderError::SourceChanged(message),
                    _ => error(message),
                }
            })?;
            videos.insert(id.clone(), video);
        }
        for cue in &plan.scene.audio {
            let sungeki_scene::AudioSource::Video { asset_id, playback } = &cue.source else {
                continue;
            };
            let id = cue.source.resource_id().expect("selected stream");
            if resources.contains_key(&id) {
                continue;
            }
            let video = &videos[asset_id];
            let decoded = video
                .decode_audio(
                    playback.audio_stream.unwrap(),
                    runner.runtime.ffmpeg(),
                    remaining(runner)?,
                    runner.flag,
                )
                .map_err(error)?;
            let AssetProbe::Video(probe) = &plan.sources[asset_id].probe else {
                unreachable!()
            };
            crate::VideoAudioSelection::new(
                playback,
                probe,
                decoded.info(),
                cue.start_us,
                sungeki_project::TimeUs(cue.end_us.0 - cue.start_us.0),
            )?;
            let path = runner
                .directory
                .join(format!("video-audio-{}.wav", resources.len()));
            std::fs::hard_link(decoded.path(), &path)?;
            resources.insert(id, serde_json::json!({"path": path}));
        }
        let mut resolver = VideoLayerResolver::new(videos);
        resolver.plan_frames(&plan.scene.layers)?;
        Ok(Self {
            resources,
            resolver,
            vrm,
        })
    }
    pub fn frame(
        &mut self,
        cache: &mut FrameCache,
        runner: &Runner<'_>,
        canvas: &Canvas,
        layers: &[&Layer],
        index: usize,
        frame_index: u64,
    ) -> Result<PathBuf, RenderError> {
        let vrm_layers = self.vrm.resolve(layers)?;
        let layers: Vec<_> = vrm_layers.iter().collect();
        let layers = layers.as_slice();
        if !layers
            .iter()
            .any(|l| matches!(l.primitive, Primitive::Video { .. }))
        {
            return cache.frame(runner, canvas, layers, index, frame_index);
        }
        // Video resolver releases the previous frame's raw resources on the
        // next resolve. Consume deferred work before that lifetime boundary.
        cache.flush(runner, canvas)?;
        let resolved = self.resolver.resolve(
            layers,
            runner.directory,
            runner.runtime.ffmpeg(),
            remaining(runner)?,
            runner.flag,
        )?;
        let mut resources = self.resources.clone();
        for (id, resource) in resolved.resources {
            resources.insert(id, serde_json::to_value(resource)?);
        }
        let path = runner
            .directory
            .join(format!("video-resources-{index}.json"));
        write_new(&path, &serde_json::to_vec(&resources)?)?;
        let layers: Vec<_> = resolved.layers.iter().collect();
        let png = cache.frame_with_resources(runner, canvas, &layers, index, frame_index, &path)?;
        cache.flush(runner, canvas)?;
        Ok(png)
    }
}
