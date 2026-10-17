//! Sungeki Irodori contract. Engine paths belong to host profiles, not voice settings.
use crate::ValidationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IrodoriSettings {
    pub voice_id: String,
    pub caption: Option<String>,
    pub cfg_scale: f64,
    pub num_steps: u16,
    /// Resolve random requests to an explicit seed before capturing job input.
    pub seed: u32,
}
impl IrodoriSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !text(&self.voice_id, 128)
            || !self.cfg_scale.is_finite()
            || !(0.0..=30.0).contains(&self.cfg_scale)
            || !(1..=200).contains(&self.num_steps)
            || self.caption.as_ref().is_some_and(|caption| {
                caption.trim().is_empty() || caption.len() > 4096 || caption.contains('\0')
            })
        {
            return Err(ValidationError::new("Irodoriの生成設定が不正です".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IrodoriIdentity {
    pub checkpoint: String,
    /// Model weights, config and tokenizer dependencies.
    pub model_manifest_sha256: String,
    /// Decoding codec weights/config are separate from the speech model.
    pub codec_manifest_sha256: String,
    pub engine_manifest_sha256: String,
    /// Voice mapping, reference audio/latents and their preparation settings.
    pub voice_manifest_sha256: String,
    pub sample_rate: u32,
    pub channels: u16,
}
impl IrodoriIdentity {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !text(&self.checkpoint, 4096)
            || [
                &self.model_manifest_sha256,
                &self.codec_manifest_sha256,
                &self.engine_manifest_sha256,
                &self.voice_manifest_sha256,
            ]
            .iter()
            .any(|value| !hash(value))
            || !(8000..=192000).contains(&self.sample_rate)
            || !(1..=2).contains(&self.channels)
        {
            return Err(ValidationError::new(
                "Irodoriの原本識別情報が不正です".into(),
            ));
        }
        Ok(())
    }
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Captured generation conditions. Fingerprint and original bytes need separate checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IrodoriAudioSource {
    pub settings: IrodoriSettings,
    pub identity: IrodoriIdentity,
    /// Model capabilities and selected registered voice metadata, without transient health.
    pub metadata_sha256: String,
}
impl IrodoriAudioSource {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.settings.validate()?;
        self.identity.validate()?;
        if !hash(&self.metadata_sha256) {
            return Err(ValidationError::new("Irodori音声の来歴が不正です".into()));
        }
        Ok(())
    }
}
