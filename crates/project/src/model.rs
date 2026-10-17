use crate::{Asset, Character, PronunciationEntry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_TIME_US: u64 = 604_800_000_000;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(transparent)]
pub struct TimeUs(#[ts(type = "number")] pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format: String,
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub canvas: Canvas,
    pub tracks: Vec<Track>,
    pub scenes: Vec<Scene>,
    pub groups: Vec<Group>,
    pub clips: Vec<Clip>,
    pub audio_mix: AudioMix,
    pub assets: Vec<Asset>,
    pub characters: Vec<Character>,
    pub pronunciation_dictionary: Vec<PronunciationEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub fps: FrameRate,
    pub background: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl FrameRate {
    pub fn frame_time(&self, frame: u64) -> Option<TimeUs> {
        if self.numerator == 0 || self.denominator == 0 {
            return None;
        }
        let time = (frame as u128) * 1_000_000 * self.denominator as u128 / self.numerator as u128;
        (time <= MAX_TIME_US as u128).then_some(TimeUs(time as u64))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub muted: bool,
    pub kind: TrackKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Visual,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fade: Option<crate::VisualFade>,
    pub id: String,
    pub name: String,
    pub start_us: TimeUs,
    pub duration_us: TimeUs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub clip_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub track_id: String,
    pub scene_id: Option<String>,
    pub start_us: TimeUs,
    pub duration_us: TimeUs,
    pub transform: Transform,
    pub content: ClipContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub motion: Option<crate::VisualMotion>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fade: Option<crate::VisualFade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub flip_x: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClipContent {
    Vrm {
        standing: crate::VrmStanding,
    },
    Sprite {
        sprites: crate::SpriteSet,
        pose: Option<String>,
        character_id: Option<String>,
    },
    /// Gaussian backdrop blur; only already composited layers inside the rectangle.
    Blur {
        sigma: f64,
    },
    Audio {
        asset_id: String,
        playback: crate::AudioPlayback,
    },
    Psd {
        asset_id: String,
        pose: crate::PsdPose,
        speech: Option<crate::PsdSpeechBinding>,
    },
    Speech {
        character_id: String,
        subtitle_text: String,
        spoken_text: Option<String>,
        audio: Option<crate::SpeechAudio>,
        // None leaves the original level unchanged; dB adjustment is applied at mix time.
        gain_db: Option<f64>,
    },
    Video {
        asset_id: String,
        playback: crate::VideoPlayback,
    },
    Image {
        asset_id: String,
    },
    Text {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        outline: Option<crate::SubtitleOutline>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        layout: Option<crate::SubtitleLayout>,
        text: String,
        font_size: f64,
        color: String,
    },
    Shape {
        shape: ShapeKind,
        fill: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioMix {
    pub normalization: Option<Normalization>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Normalization {
    pub target_lufs: f64,
    pub true_peak_db: f64,
}

impl Project {
    pub fn empty(id: String, title: String) -> Self {
        Self {
            format: "vvm-project".into(),
            schema_version: 1,
            id,
            title,
            canvas: Canvas {
                width: 1920,
                height: 1080,
                fps: FrameRate {
                    numerator: 30,
                    denominator: 1,
                },
                background: "#ffffff".into(),
            },
            tracks: vec![],
            scenes: vec![],
            groups: vec![],
            clips: vec![],
            assets: vec![],
            characters: vec![],
            pronunciation_dictionary: vec![],
            audio_mix: AudioMix {
                normalization: None,
            },
        }
    }
    pub fn duration(&self) -> Option<TimeUs> {
        self.clips
            .iter()
            .map(|c| (c.start_us, c.duration_us))
            .chain(self.scenes.iter().map(|s| (s.start_us, s.duration_us)))
            .try_fold(0, |max, (start, duration)| {
                start.0.checked_add(duration.0).map(|end| max.max(end))
            })
            .filter(|time| *time <= MAX_TIME_US)
            .map(TimeUs)
    }
}
