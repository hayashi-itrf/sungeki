#[path = "qwen_audio.rs"]
mod audio;
use crate::ValidationError;
pub use audio::{QwenAudioSource, QwenSelection};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QwenSettings {
    pub model: String,
    pub voice: String,
    pub speed: f64,
    pub language: String,
    pub instruct: Option<String>,
}
impl QwenSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for value in [&self.model, &self.voice, &self.language] {
            if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
                return Err(ValidationError::new("Qwenの音声設定が不正です".into()));
            }
        }
        if !self.speed.is_finite()
            || !(0.25..=4.).contains(&self.speed)
            || self
                .instruct
                .as_ref()
                .is_some_and(|v| v.len() > 4096 || v.contains('\0'))
        {
            return Err(ValidationError::new("Qwenの音声設定が不正です".into()));
        }
        Ok(())
    }
}
