use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Declared identity to compare against inspected runtime files before dispatch.
/// Merely deserializing this value does not verify any file or running process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2Identity {
    pub engine_revision: String,
    pub model_sha256: String,
    pub config_sha256: String,
    pub style_vectors_sha256: String,
    /// Hash of the text encoder/tokenizer dependency manifest, not a display name.
    pub text_encoder_fingerprint: String,
}
impl Sbv2Identity {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.engine_revision.trim().is_empty()
            || self.engine_revision.len() > 256
            || self.engine_revision.chars().any(char::is_control)
        {
            return Err(ValidationError::new(
                "SBV2エンジンのrevisionが不正です".into(),
            ));
        }
        for hash in [
            &self.model_sha256,
            &self.config_sha256,
            &self.style_vectors_sha256,
            &self.text_encoder_fingerprint,
        ] {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(ValidationError::new(
                    "SBV2の原本識別値は小文字のSHA-256で指定してください".into(),
                ));
            }
        }
        Ok(())
    }
}
