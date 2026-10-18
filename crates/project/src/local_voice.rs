//! Explicit controls for local HTTP engines. Endpoints belong to runtime settings.
use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum LocalSpeechEngine {
    AivisSpeech,
    Coeiroink,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "engine", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalVoiceSettings {
    AivisSpeech {
        speaker_uuid: String,
        style_id: i32,
        speed_scale: f64,
        pitch_scale: f64,
        emotion_scale: f64,
        tempo_dynamics_scale: f64,
        volume_scale: f64,
        pre_phoneme_length: f64,
        post_phoneme_length: f64,
    },
    Coeiroink {
        speaker_uuid: String,
        style_id: i32,
        speed_scale: f64,
        pitch_scale: f64,
        intonation_scale: f64,
        volume_scale: f64,
        pre_phoneme_length: f64,
        post_phoneme_length: f64,
    },
}
impl LocalVoiceSettings {
    pub fn engine(&self) -> LocalSpeechEngine {
        match self {
            Self::AivisSpeech { .. } => LocalSpeechEngine::AivisSpeech,
            Self::Coeiroink { .. } => LocalSpeechEngine::Coeiroink,
        }
    }
    pub fn selection(&self) -> (&str, i32) {
        match self {
            Self::AivisSpeech {
                speaker_uuid,
                style_id,
                ..
            }
            | Self::Coeiroink {
                speaker_uuid,
                style_id,
                ..
            } => (speaker_uuid, *style_id),
        }
    }
    pub fn validate(&self) -> Result<(), ValidationError> {
        let (speaker, _) = self.selection();
        if speaker.len() != 36
            || speaker.bytes().enumerate().any(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b != b'-'
                } else {
                    !b.is_ascii_hexdigit()
                }
            })
        {
            return Err(ValidationError::new("話者UUIDが不正です".into()));
        }
        let (speed, pitch, volume, pre, post) = match self {
            Self::AivisSpeech {
                speed_scale,
                pitch_scale,
                emotion_scale,
                tempo_dynamics_scale,
                volume_scale,
                pre_phoneme_length,
                post_phoneme_length,
                ..
            } => {
                range("感情表現", *emotion_scale, 0., 2.)?;
                range("テンポの緩急", *tempo_dynamics_scale, 0., 2.)?;
                (
                    *speed_scale,
                    *pitch_scale,
                    *volume_scale,
                    *pre_phoneme_length,
                    *post_phoneme_length,
                )
            }
            Self::Coeiroink {
                speed_scale,
                pitch_scale,
                intonation_scale,
                volume_scale,
                pre_phoneme_length,
                post_phoneme_length,
                style_id,
                ..
            } => {
                if *style_id < 0 {
                    return Err(ValidationError::new("スタイルIDが不正です".into()));
                }
                range("抑揚", *intonation_scale, 0., 2.)?;
                (
                    *speed_scale,
                    *pitch_scale,
                    *volume_scale,
                    *pre_phoneme_length,
                    *post_phoneme_length,
                )
            }
        };
        range("話速", speed, 0.5, 2.)?;
        range("音高", pitch, -0.15, 0.15)?;
        range("音量", volume, 0., 2.)?;
        range("前の無音", pre, 0., 1.5)?;
        range("後の無音", post, 0., 1.5)
    }
}
fn range(name: &str, value: f64, min: f64, max: f64) -> Result<(), ValidationError> {
    if !value.is_finite() || !(min..=max).contains(&value) {
        Err(ValidationError::new(format!(
            "{name}は{min}〜{max}で指定してください"
        )))
    } else {
        Ok(())
    }
}
