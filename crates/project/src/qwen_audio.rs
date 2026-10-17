use crate::{QwenIdentity, ValidationError};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
pub struct QwenSelection {
    pub backend: String,
    pub model_id: String,
    pub request_model: String,
    pub voice: String,
    pub language: String,
    pub metadata_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct QwenAudioSource {
    pub selection: QwenSelection,
    pub identity: QwenIdentity,
}
impl QwenAudioSource {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.identity.validate()?;
        let s = &self.selection;
        if [
            &s.backend,
            &s.model_id,
            &s.request_model,
            &s.voice,
            &s.language,
        ]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 4096 || s.chars().any(char::is_control))
            || s.backend != self.identity.backend
            || s.model_id != self.identity.model_id
            || s.metadata_sha256.len() != 64
            || !s
                .metadata_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ValidationError::new("Qwen音声の来歴が不正です".into()));
        }
        Ok(())
    }
}
