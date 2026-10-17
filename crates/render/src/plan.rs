#[path = "preview_plan.rs"]
mod preview;
#[cfg(unix)]
pub use preview::PreviewArtifact;
pub use preview::PreviewPlan;
use std::collections::BTreeMap;
use sungeki_project::{Asset, AssetProbe, Project};
use sungeki_scene::{AudioSource, Primitive, ScenePlan};

#[derive(Debug)]
pub enum RenderError {
    Message(String),
    /// The owned worker has exited, or cancellation was observed between workers.
    Settled(String),
    Cancelled,
    SourceChanged(String),
    TextLayout(Vec<crate::TextLayoutIssue>),
}
impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("動画出力を取り消しました"),
            Self::TextLayout(issues) => write!(f, "文字のはみ出しを{}件検出しました", issues.len()),
            Self::Message(message) | Self::SourceChanged(message) | Self::Settled(message) => {
                f.write_str(message)
            }
        }
    }
}
impl std::error::Error for RenderError {}
impl From<std::io::Error> for RenderError {
    fn from(value: std::io::Error) -> Self {
        Self::Message(value.to_string())
    }
}
impl From<serde_json::Error> for RenderError {
    fn from(value: serde_json::Error) -> Self {
        Self::Message(value.to_string())
    }
}
pub(crate) fn error(value: impl std::fmt::Display) -> RenderError {
    RenderError::Message(value.to_string())
}

/// Immutable and bounded. Freshness must be checked against this exact Project
/// by the host; structural validation cannot establish synthesis freshness.
#[derive(Debug)]
pub struct RenderPlan {
    pub(crate) scene: ScenePlan,
    pub(crate) sources: BTreeMap<String, Asset>,
    pub(crate) chapters: Vec<crate::chapters::Chapter>,
}
impl RenderPlan {
    pub fn new(
        project: &Project,
        check_speech: impl Fn(&str) -> Result<(), RenderError>,
    ) -> Result<Self, RenderError> {
        let scene = ScenePlan::new(project).map_err(error)?;
        Self::from_scene(project, scene, check_speech)
    }
    pub(crate) fn from_scene(
        project: &Project,
        scene: ScenePlan,
        check_speech: impl Fn(&str) -> Result<(), RenderError>,
    ) -> Result<Self, RenderError> {
        if scene.frame_count == 0
            || scene.duration_us.0 > 7_200_000_000
            || scene.frame_count > 432_000
            || scene.canvas.width % 2 != 0
            || scene.canvas.height % 2 != 0
            || scene.canvas.width > 4096
            || scene.canvas.height > 4096
            || scene.audio.len() > 128
            // Per-frame motion needs more than the former static-scene limit.
            // Keep a finite job budget aligned with scene effect expansion.
            || scene.visual_spans().take(100_001).count() > 100_000
        {
            return Err(error(
                "出力範囲外です（空作品・2時間/432000frame超・奇数寸法・4096px超・音声128/描画区間100000超）",
            ));
        }
        if scene
            .audio_mix
            .normalization
            .as_ref()
            .is_some_and(|n| n.target_lufs <= -70.)
        {
            return Err(error(
                "全体音量の目標は測定下限の-70 LUFSより大きくしてください",
            ));
        }
        let mut sources = BTreeMap::new();
        let mut add = |id: &str| -> Result<&Asset, RenderError> {
            let asset = project
                .assets
                .iter()
                .find(|a| a.id == id)
                .ok_or_else(|| error("素材参照がありません"))?;
            sources.insert(id.to_owned(), asset.clone());
            Ok(asset)
        };
        for layer in &scene.layers {
            if let Primitive::Vrm {
                model_asset_id,
                motion_asset_id,
                ..
            } = &layer.primitive
            {
                for (id, kind) in [
                    (model_asset_id, sungeki_project::VrmKind::Model),
                    (motion_asset_id, sungeki_project::VrmKind::Motion),
                ] {
                    if !matches!(&add(id)?.probe, AssetProbe::Vrm(p) if p.inspection.kind == kind) {
                        return Err(error("VRM原本の種類が不一致です"));
                    }
                }
            }
            if let Primitive::Video { asset_id, .. } = &layer.primitive {
                if !matches!(add(asset_id)?.probe, AssetProbe::Video(_)) {
                    return Err(error("動画原本の検査記録がありません"));
                }
            }
            if let Primitive::Psd { asset_id, pose } = &layer.primitive {
                let AssetProbe::Psd(probe) = &add(asset_id)?.probe else {
                    return Err(error("PSD原本の検査記録がありません"));
                };
                pose.validate(probe).map_err(error)?;
            }
            if let Primitive::Image { asset_id } = &layer.primitive {
                if !matches!(add(asset_id)?.probe, AssetProbe::Image(_)) {
                    return Err(error("画像原本の検査記録がありません"));
                }
            }
        }
        for cue in &scene.audio {
            let id = cue
                .source
                .asset_id()
                .ok_or_else(|| error(format!("未合成の音声: {}", cue.clip_id)))?;
            let asset = add(id)?;
            if let AudioSource::Video { playback, .. } = &cue.source {
                let AssetProbe::Video(probe) = &asset.probe else {
                    return Err(error("動画原本の検査記録がありません"));
                };
                crate::VideoAudioSelection::from_probe(
                    playback,
                    probe,
                    cue.start_us,
                    sungeki_project::TimeUs(cue.end_us.0 - cue.start_us.0),
                )?;
                continue;
            }
            let AssetProbe::Audio(probe) = &asset.probe else {
                return Err(error("音声原本の検査記録がありません"));
            };
            match &cue.source {
                AudioSource::Video { .. } => unreachable!("video audio validated above"),
                AudioSource::Speech { .. } => {
                    check_speech(&cue.clip_id)?;
                    if cue.end_us.0 - cue.start_us.0 < probe.duration_us.0 {
                        return Err(error(format!(
                            "音声がclipの尺を超えています: {}",
                            cue.clip_id
                        )));
                    }
                }
                AudioSource::Material { playback, .. } => playback
                    .validate(
                        probe,
                        sungeki_project::TimeUs(cue.end_us.0 - cue.start_us.0),
                    )
                    .map_err(error)?,
            }
        }
        let bytes = sources
            .values()
            .map(|a| match &a.probe {
                AssetProbe::Image(p) => p.byte_length,
                AssetProbe::Audio(p) => p.byte_length,
                AssetProbe::Psd(p) => p.byte_length,
                AssetProbe::Video(p) => p.byte_length,
                AssetProbe::Vrm(p) => p.byte_length,
            })
            .try_fold(0u64, |total, size| total.checked_add(size))
            .ok_or_else(|| error("使用素材の合計サイズが不正です"))?;
        if bytes > 1024 * 1024 * 1024 {
            return Err(error("使用素材の合計が1GiBを超えています"));
        }
        let end_ms =
            (u128::from(scene.frame_count) * u128::from(scene.canvas.fps.denominator) * 1000)
                / u128::from(scene.canvas.fps.numerator);
        let chapters = crate::chapters::plan(project, end_ms as u64);
        let result = Self {
            scene,
            sources,
            chapters,
        };
        #[cfg(unix)]
        if result.scene.audio_mix.normalization.is_some() {
            sungeki_media::MixFrameLimit::for_video(
                result.scene.duration_us,
                &result.scene.canvas.fps,
                true,
            )
            .map_err(error)?;
        }
        crate::audio_graph::validate_loop_memory(&result)?;
        Ok(result)
    }
    pub fn frame_count(&self) -> u64 {
        self.scene.frame_count
    }
    pub fn audio_samples(&self) -> u64 {
        (u128::from(self.scene.frame_count) * u128::from(self.scene.canvas.fps.denominator) * 48000)
            .div_ceil(u128::from(self.scene.canvas.fps.numerator)) as u64
    }
}
