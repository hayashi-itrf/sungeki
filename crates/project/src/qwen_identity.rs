use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QwenIdentity {
    pub backend: String,
    pub model_id: String,
    /// Canonical manifest of all model weights, configs and tokenizer dependencies.
    pub model_manifest_sha256: String,
    /// Engine sources and pinned execution environment manifest.
    pub engine_manifest_sha256: String,
    /// Selected voice mapping, and all reference audio/text for a cloned voice.
    pub voice_manifest_sha256: String,
    pub sample_rate: u32,
    pub channels: u16,
}
impl QwenIdentity {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for value in [&self.backend, &self.model_id] {
            if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control)
            {
                return Err(ValidationError::new(
                    "Qwenのモデル識別情報が不正です".into(),
                ));
            }
        }
        for hash in [
            &self.model_manifest_sha256,
            &self.engine_manifest_sha256,
            &self.voice_manifest_sha256,
        ] {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(ValidationError::new(
                    "Qwenのモデル識別情報が不正です".into(),
                ));
            }
        }
        if !(8000..=192000).contains(&self.sample_rate) || !(1..=2).contains(&self.channels) {
            return Err(ValidationError::new(
                "Qwenのモデル識別情報が不正です".into(),
            ));
        }
        Ok(())
    }
}
