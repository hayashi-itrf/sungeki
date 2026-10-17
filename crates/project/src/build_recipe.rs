//! Typed input for new-session production. No clip IDs or speech timing are
//! required from callers. File/preset freshness is checked by the host later.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildRecipe {
    pub preset_id: String,
    pub title: String,
    pub assets: Vec<Asset>,
    pub roles: Vec<BuildRole>,
    pub scenes: Vec<BuildScene>,
    pub bgm: Option<BuildBgm>,
    pub foreword: Option<BuildForeword>,
    pub audio_mix: AudioMix,
    pub pronunciation_dictionary: Vec<PronunciationEntry>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildRole {
    pub role: String,
    pub preset_id: String,
    pub character: Character,
    pub standing: BuildStanding,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BuildStanding {
    Vrm {
        standing: VrmStanding,
        flip_x: bool,
    },
    Sprite {
        sprites: SpriteSet,
        pose: Option<String>,
        flip_x: bool,
    },
    Image {
        asset_id: String,
        flip_x: bool,
    },
    Psd {
        asset_id: String,
        pose: PsdPose,
        speech: PsdSpeechBinding,
        flip_x: bool,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildScene {
    /// Applied after synthesis resolves the actual scene duration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fade: Option<VisualFade>,
    pub name: String,
    pub background_asset_id: String,
    pub gap_us: TimeUs,
    pub lines: Vec<BuildLine>,
    pub diagrams: Vec<BuildDiagram>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildLine {
    /// Sprite pose change for this role, held until changed; resets each scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sprite_pose: Option<String>,
    pub role: String,
    pub subtitle_text: String,
    pub spoken_text: Option<String>,
    pub pose: Option<PsdPose>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildDiagram {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fade: Option<VisualFade>,
    /// Image movement over the resolved inclusive speech range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub motion: Option<VisualMotion>,
    /// Image reaction, timed to the inclusive speech range after synthesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reaction: Option<BuildReaction>,
    pub asset_id: String,
    /// None selects an image. Video requires an explicit source/audio selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub video_playback: Option<VideoPlayback>,
    /// Inclusive zero-based line range in this scene, resolved after synthesis.
    pub first_line: u16,
    pub last_line: u16,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildBgm {
    /// Optional source clip duration for proportionally retiming volume points only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub volume_point_reference_duration_us: Option<TimeUs>,
    pub asset_id: String,
    pub playback: AudioPlayback,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildForeword {
    pub preset_id: String,
    pub text: String,
    pub duration_us: TimeUs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildReaction {
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub start_scale: f64,
    pub fade: VisualFade,
}
impl BuildReaction {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if ![
            self.center_x,
            self.center_y,
            self.width,
            self.height,
            self.start_scale,
        ]
        .iter()
        .all(|v| v.is_finite())
            || self.width <= 0.
            || self.height <= 0.
            || self.start_scale <= 0.
            || self.start_scale > 1.
        {
            return Err(ValidationError::new(
                "リアクションの寸法・開始倍率が不正です".into(),
            ));
        }
        self.fade.validate(TimeUs(MAX_TIME_US))
    }
}
