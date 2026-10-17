use crate::{AssetProbe, Project, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Reference to a persistent original plus the synthesis input that produced it.
/// Metadata is not proof of current file contents or current speech input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechAudio {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub irodori_source: Option<crate::IrodoriAudioSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub qwen_source: Option<crate::QwenAudioSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sbv2_source: Option<crate::Sbv2AudioSource>,
    pub asset_id: String,
    pub input_fingerprint: String,
    pub engine_version: String,
    pub lip_sync: Option<crate::LipSyncAnalysis>,
}
impl SpeechAudio {
    pub(crate) fn validate(&self, project: &Project) -> Result<(), ValidationError> {
        let probe = project
            .assets
            .iter()
            .find_map(|asset| {
                if asset.id != self.asset_id {
                    return None;
                }
                if let AssetProbe::Audio(probe) = &asset.probe {
                    Some(probe)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                ValidationError::new("合成音声の素材が見つからないか音声以外です".into())
            })?;
        if let Some(lip_sync) = &self.lip_sync {
            lip_sync.validate(probe)?;
        }
        if let Some(source) = &self.irodori_source {
            source.validate()?;
            if self.sbv2_source.is_some()
                || self.qwen_source.is_some()
                || source.identity.sample_rate != probe.sample_rate
                || source.identity.channels != probe.channels
                || source.identity.engine_manifest_sha256 != self.engine_version
            {
                return Err(ValidationError::new(
                    "Irodori音声の来歴と原本形式が一致しません".into(),
                ));
            }
        }
        if let Some(source) = &self.qwen_source {
            source.validate()?;
            if self.sbv2_source.is_some()
                || source.identity.sample_rate != probe.sample_rate
                || source.identity.channels != probe.channels
                || source.identity.engine_manifest_sha256 != self.engine_version
            {
                return Err(ValidationError::new(
                    "Qwen音声の来歴と原本形式が一致しません".into(),
                ));
            }
        }
        if let Some(source) = &self.sbv2_source {
            source.validate()?;
            if source.sample_rate != probe.sample_rate
                || source.channels != probe.channels
                || source.identity.engine_revision != self.engine_version
            {
                return Err(ValidationError::new(
                    "SBV2音声の来歴と原本形式が一致しません".into(),
                ));
            }
        }
        if self.input_fingerprint.len() != 64
            || !self
                .input_fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.engine_version.trim().is_empty()
            || self.engine_version.len() > if self.sbv2_source.is_some() { 256 } else { 128 }
            || self.engine_version.chars().any(char::is_control)
        {
            return Err(ValidationError::new(
                "合成音声の入力識別情報が不正です".into(),
            ));
        }
        Ok(())
    }
}
