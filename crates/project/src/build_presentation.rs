use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fitting: Option<BuildVisualFits>,
    /// Ordered sections. Body and collected credits are required; omitted optional sections are hidden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub structure: Option<BuildStructure>,
    /// Fixed, credited media and authored mouth cues, before synthesized scenes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub opening: Option<BuildOpening>,
    /// Exact static rectangles, resolved before the first layout is committed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slots: Option<BuildLayoutSlots>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ending: Option<BuildEnding>,
    /// Offset from the preset's fitted standing rectangle, in canvas pixels.
    pub standing_offset_y: f64,
    pub credits: Option<BuildCreditsStyle>,
    pub bgm_speech_envelope: Option<BuildSpeechEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum BuildBlock {
    Foreword,
    Opening,
    Body,
    Credits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildStructure {
    pub blocks: Vec<BuildBlock>,
    /// Applies only to the plain title card, not to authored opening audio.
    pub title_duration_us: Option<TimeUs>,
}
impl BuildStructure {
    pub fn validate(&self, recipe: &BuildRecipe) -> Result<(), ValidationError> {
        use BuildBlock::*;
        let unique: std::collections::HashSet<_> = self.blocks.iter().collect();
        if self.blocks.len() != unique.len()
            || !self.blocks.contains(&Body)
            || !self.blocks.contains(&Credits)
            || (self.blocks.contains(&Foreword) && recipe.foreword.is_none())
            || self
                .title_duration_us
                .is_some_and(|t| t.0 == 0 || t.0 > 60_000_000)
            || (self.title_duration_us.is_some()
                && recipe
                    .presentation
                    .as_ref()
                    .is_some_and(|p| p.opening.is_some()))
        {
            return Err(ValidationError::new(
                "制作の構成順・タイトル尺が不正です。本編とクレジットは必要です".into(),
            ));
        }
        if recipe
            .bgm
            .as_ref()
            .is_some_and(|b| !b.playback.volume_points.is_empty())
        {
            return Err(ValidationError::new("構成を変更するときはBGMの固定時刻ポイントを解除し、発話に連動する音量を指定してください".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildLayoutSlots {
    /// Keys are recipe role names. Unspecified roles keep their preset rectangle.
    pub standing: std::collections::BTreeMap<String, Transform>,
    pub subtitle: Option<Transform>,
    pub diagram: Option<Transform>,
    pub diagrams_behind_standings: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildEnding {
    pub duration_us: TimeUs,
    /// Full-canvas image. None retains the credit card's solid background.
    pub background_asset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildCreditsStyle {
    pub background: String,
    pub color: String,
    pub font_size: f64,
    pub transform: Transform,
    pub layout: SubtitleLayout,
    pub fade: VisualFade,
    /// Explicit imported display policy; asset/character attribution is retained.
    pub omit_lines_containing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildSpeechEnvelope {
    /// Music starts at project zero, including the optional foreword.
    /// Level is 1 outside the synthesized first-to-last speech interval.
    pub speech_level: f64,
    pub ramp_us: TimeUs,
}

impl BuildPresentation {
    pub fn validate(&self, recipe: &BuildRecipe) -> Result<(), ValidationError> {
        if let Some(fitting) = &self.fitting {
            fitting.validate(recipe, self)?;
        }
        if let Some(structure) = &self.structure {
            structure.validate(recipe)?;
        }
        if let Some(opening) = &self.opening {
            opening.validate(recipe)?;
        }
        let invalid = || ValidationError::new("制作の配置・クレジット・BGM設定が不正です".into());
        if let Some(slots) = &self.slots {
            for (name, transform) in &slots.standing {
                if !recipe.roles.iter().any(|r| &r.role == name) {
                    return Err(ValidationError::new(format!(
                        "配置の役名が未登録です: {name}"
                    )));
                }
                static_rect(transform)?;
            }
            if let Some(t) = &slots.subtitle {
                static_rect(t)?;
                if t.flip_x.unwrap_or(false) {
                    return Err(invalid());
                }
            }
            if let Some(t) = &slots.diagram {
                static_rect(t)?;
                if recipe
                    .scenes
                    .iter()
                    .flat_map(|s| &s.diagrams)
                    .any(|d| d.motion.is_some() || d.reaction.is_some())
                {
                    return Err(ValidationError::new(
                        "説明図の固定配置と移動・リアクションは併用できません".into(),
                    ));
                }
            }
            // An exact standing rectangle and a second global offset are ambiguous.
            if !slots.standing.is_empty() && self.standing_offset_y != 0. {
                return Err(invalid());
            }
            if recipe.preset_id == "narration-portrait-v1" {
                return Err(ValidationError::new(
                    "配置枠の指定は横長制作で使用してください".into(),
                ));
            }
        }
        let ending_duration = self
            .ending
            .as_ref()
            .map_or(TimeUs(3_500_000), |e| e.duration_us);
        if let Some(ending) = &self.ending {
            if !(1..=60_000_000).contains(&ending.duration_us.0) {
                return Err(invalid());
            }
            if let Some(id) = &ending.background_asset_id {
                let asset = recipe
                    .assets
                    .iter()
                    .find(|a| &a.id == id)
                    .ok_or_else(invalid)?;
                if !matches!(asset.probe, AssetProbe::Image(_)) || asset.credit.trim().is_empty() {
                    return Err(invalid());
                }
            }
        }
        if !self.standing_offset_y.is_finite() || self.standing_offset_y.abs() > 16384. {
            return Err(invalid());
        }
        if let Some(c) = &self.credits {
            let t = &c.transform;
            if !crate::validate::color(&c.background)
                || !crate::validate::color(&c.color)
                || !c.font_size.is_finite()
                || !(1. ..=512.).contains(&c.font_size)
                || ![t.x, t.y, t.width, t.height, t.opacity]
                    .iter()
                    .all(|v| v.is_finite())
                || t.width <= 0.
                || t.height <= 0.
                || !(0. ..=1.).contains(&t.opacity)
                || t.motion.is_some()
                || t.fade.is_some()
                || c.omit_lines_containing.len() > 32
                || c.omit_lines_containing.iter().any(|s| {
                    s.trim().is_empty() || s.len() > 1024 || s.contains(['\0', '\n', '\r'])
                })
            {
                return Err(invalid());
            }
            c.fade.validate(ending_duration)?;
        }
        if let Some(e) = &self.bgm_speech_envelope {
            let bgm = recipe.bgm.as_ref().ok_or_else(invalid)?;
            if !e.speech_level.is_finite()
                || !(0. ..=1.).contains(&e.speech_level)
                || e.ramp_us.0 == 0
                || e.ramp_us.0 > 60_000_000
                || !bgm.playback.volume_points.is_empty()
                || bgm.playback.ducking.is_some()
                || bgm.volume_point_reference_duration_us.is_some()
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

pub(super) fn static_rect(t: &Transform) -> Result<(), ValidationError> {
    if ![t.x, t.y, t.width, t.height, t.opacity]
        .iter()
        .all(|v| v.is_finite())
        || t.x.abs() > 16384.
        || t.y.abs() > 16384.
        || !(0. ..=16384.).contains(&t.width)
        || t.width == 0.
        || !(0. ..=16384.).contains(&t.height)
        || t.height == 0.
        || !(0. ..=1.).contains(&t.opacity)
        || t.motion.is_some()
        || t.fade.is_some()
    {
        return Err(ValidationError::new("制作の配置枠が不正です".into()));
    }
    Ok(())
}
