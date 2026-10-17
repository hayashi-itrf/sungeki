use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2Settings {
    pub model_id: u32,
    pub speaker_id: u32,
    pub language: Sbv2Language,
    pub style: String,
    pub style_weight: f64,
    /// Duration multiplier: larger values speak more slowly.
    pub length: f64,
    pub sdp_ratio: f64,
    pub noise: f64,
    pub noisew: f64,
    pub auto_split: bool,
    pub split_interval: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub enum Sbv2Language {
    JP,
    EN,
    ZH,
}
impl Sbv2Settings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.style.trim().is_empty()
            || self.style.len() > 1024
            || self.style.chars().any(char::is_control)
        {
            return Err(ValidationError::new(
                "SBV2のスタイルまたは音声設定が不正です".into(),
            ));
        }
        for (value, low, high) in [
            (self.style_weight, 0., 100.),
            (self.length, 0.1, 10.),
            (self.sdp_ratio, 0., 1.),
            (self.noise, 0., 10.),
            (self.noisew, 0., 10.),
            (self.split_interval, 0., 60.),
        ] {
            if !value.is_finite() || !(low..=high).contains(&value) {
                return Err(ValidationError::new(
                    "SBV2のスタイルまたは音声設定が不正です".into(),
                ));
            }
        }
        Ok(())
    }
}
