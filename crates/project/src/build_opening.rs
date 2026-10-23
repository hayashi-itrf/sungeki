//! Fixed opening media. Authored mouth cues are not claimed as TTS analysis.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildOpening {
    pub duration_us: TimeUs,
    pub background_asset_id: String,
    pub logo: Option<BuildOpeningImage>,
    pub audio: Vec<BuildOpeningAudio>,
    pub performers: Vec<BuildOpeningPerformer>,
    pub theme: BuildOpeningText,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildOpeningRange {
    pub start_us: TimeUs,
    pub duration_us: TimeUs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildOpeningImage {
    pub asset_id: String,
    pub range: BuildOpeningRange,
    pub transform: Transform,
    pub fade: Option<VisualFade>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildOpeningAudio {
    pub asset_id: String,
    pub range: BuildOpeningRange,
    pub playback: AudioPlayback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildOpeningPerformer {
    pub role: String,
    pub transform: Transform,
    /// Sorted, disjoint open-mouth intervals; the remaining opening is closed.
    pub mouth_open: Vec<BuildOpeningRange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildOpeningText {
    pub text: String,
    pub range: BuildOpeningRange,
    pub transform: Transform,
    pub font_size: f64,
    pub color: String,
    pub outline: Option<SubtitleOutline>,
    pub layout: SubtitleLayout,
}

impl BuildOpeningRange {
    pub fn end(&self) -> u64 {
        self.start_us.0.saturating_add(self.duration_us.0)
    }
    fn validate(&self, duration: TimeUs) -> Result<(), ValidationError> {
        require(
            self.duration_us.0 > 0 && self.end() <= duration.0,
            "表示区間",
        )
    }
}
impl BuildOpeningPerformer {
    /// Partition the whole opening, including silent gaps, without overlaps.
    pub fn intervals(&self, duration: TimeUs) -> Vec<(BuildOpeningRange, bool)> {
        let mut result = Vec::new();
        let mut cursor = 0;
        for open in &self.mouth_open {
            if open.start_us.0 > cursor {
                result.push((
                    BuildOpeningRange {
                        start_us: TimeUs(cursor),
                        duration_us: TimeUs(open.start_us.0 - cursor),
                    },
                    false,
                ));
            }
            result.push((open.clone(), true));
            cursor = open.end();
        }
        if cursor < duration.0 {
            result.push((
                BuildOpeningRange {
                    start_us: TimeUs(cursor),
                    duration_us: TimeUs(duration.0 - cursor),
                },
                false,
            ));
        }
        result
    }
}
fn require(ok: bool, field: &str) -> Result<(), ValidationError> {
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new(format!("冒頭構成の{field}が不正です")))
    }
}
impl BuildOpening {
    pub fn validate(&self, recipe: &BuildRecipe) -> Result<(), ValidationError> {
        require((1..=120_000_000).contains(&self.duration_us.0), "尺")?;
        require(
            recipe.preset_id != "narration-portrait-v1",
            "横長プリセット",
        )?;
        let asset = |id: &str| {
            recipe
                .assets
                .iter()
                .find(|a| a.id == id && !a.credit.trim().is_empty())
                .ok_or_else(|| {
                    ValidationError::new("冒頭構成に帰属表記付きの登録素材が必要です".into())
                })
        };
        let image = |id: &str| require(matches!(asset(id)?.probe, AssetProbe::Image(_)), "画像");
        image(&self.background_asset_id)?;
        if let Some(logo) = &self.logo {
            image(&logo.asset_id)?;
            logo.range.validate(self.duration_us)?;
            super::presentation::static_rect(&logo.transform)?;
            if let Some(fade) = &logo.fade {
                fade.validate(logo.range.duration_us)?;
            }
        }
        require(!self.audio.is_empty() && self.audio.len() <= 16, "音声数")?;
        for audio in &self.audio {
            audio.range.validate(self.duration_us)?;
            let AssetProbe::Audio(probe) = &asset(&audio.asset_id)?.probe else {
                return Err(ValidationError::new(
                    "冒頭の発声には音声原本が必要です".into(),
                ));
            };
            require(
                !audio.playback.looped && audio.playback.ducking.is_none(),
                "発声音声の再生設定",
            )?;
            audio.playback.validate(probe, audio.range.duration_us)?;
        }
        require(
            !self.performers.is_empty() && self.performers.len() <= recipe.roles.len(),
            "出演者数",
        )?;
        let mut roles = std::collections::HashSet::new();
        for performer in &self.performers {
            require(roles.insert(&performer.role), "出演者の重複")?;
            let role = recipe
                .roles
                .iter()
                .find(|r| r.role == performer.role)
                .ok_or_else(|| ValidationError::new("冒頭の出演者が未登録です".into()))?;
            require(
                matches!(role.standing, BuildStanding::Psd { .. }),
                "PSD出演者",
            )?;
            super::presentation::static_rect(&performer.transform)?;
            require(performer.mouth_open.len() <= 256, "口の動きの数")?;
            let mut end = 0;
            for range in &performer.mouth_open {
                range.validate(self.duration_us)?;
                require(range.start_us.0 >= end, "口の動きの順序・重複")?;
                end = range.end();
            }
        }
        let t = &self.theme;
        t.range.validate(self.duration_us)?;
        super::presentation::static_rect(&t.transform)?;
        require(
            !t.text.trim().is_empty() && t.text.len() <= 4096 && !t.text.contains('\0'),
            "テーマ文",
        )?;
        require(
            t.font_size.is_finite()
                && (1. ..=512.).contains(&t.font_size)
                && crate::validate::color(&t.color),
            "文字スタイル",
        )?;
        if let Some(outline) = &t.outline {
            require(
                outline.width <= 32 && crate::validate::color(&outline.color),
                "文字の縁取り",
            )?;
        }
        Ok(())
    }
}
