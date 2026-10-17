use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Explicit controls; no engine-specific optional-field bag or legacy defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VoicevoxSettings {
    pub style_id: u32,
    pub speed_scale: f64,
    pub pitch_scale: f64,
    pub intonation_scale: f64,
    pub volume_scale: f64,
    pub pre_phoneme_length: f64,
    pub post_phoneme_length: f64,
    pub pause_length: Option<f64>,
    pub pause_length_scale: f64,
    pub output_sampling_rate: u32,
    pub output_stereo: bool,
    pub enable_interrogative_upspeak: bool,
}
impl VoicevoxSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        finite("話速", self.speed_scale, Some(0.0))?;
        if self.speed_scale == 0.0 {
            return Err(ValidationError::new("話速は0より大きい値が必要です".into()));
        }
        finite("音高", self.pitch_scale, None)?;
        finite("抑揚", self.intonation_scale, None)?;
        for (name, value) in [
            ("音量", self.volume_scale),
            ("開始前の無音", self.pre_phoneme_length),
            ("終了後の無音", self.post_phoneme_length),
            ("句読点の無音倍率", self.pause_length_scale),
        ] {
            finite(name, value, Some(0.0))?;
        }
        if let Some(value) = self.pause_length {
            finite("句読点の無音", value, Some(0.0))?;
        }
        if self.output_sampling_rate == 0 {
            return Err(ValidationError::new(
                "サンプリングレートは0にできません".into(),
            ));
        }
        Ok(())
    }
}

fn finite(name: &str, value: f64, minimum: Option<f64>) -> Result<(), ValidationError> {
    if !value.is_finite() || minimum.is_some_and(|min| value < min) {
        return Err(ValidationError::new(format!("{name}の値が不正です")));
    }
    Ok(())
}
