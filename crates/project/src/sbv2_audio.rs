use crate::{Sbv2Identity, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2Selection {
    pub model_id: u32,
    pub speaker_id: u32,
    pub speaker_name: String,
    pub style: String,
    pub style_id: u32,
    pub metadata_sha256: String,
}
/// Captured synthesis provenance, not an attestation of current files or processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2AudioSource {
    pub selection: Sbv2Selection,
    pub identity: Sbv2Identity,
    pub sample_rate: u32,
    pub channels: u16,
}
impl Sbv2AudioSource {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.identity.validate()?;
        let s = &self.selection;
        if [&s.speaker_name, &s.style]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 1024 || s.chars().any(char::is_control))
            || s.metadata_sha256.len() != 64
            || !s
                .metadata_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !(8000..=192000).contains(&self.sample_rate)
            || self.channels != 1
        {
            return Err(ValidationError::new("SBV2音声の来歴が不正です".into()));
        }
        Ok(())
    }
}
