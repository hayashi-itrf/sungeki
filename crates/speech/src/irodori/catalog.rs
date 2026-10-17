//! Sungeki metadata contract, independent of the previous application's HTTP schema.
use crate::InputError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use sungeki_project::{IrodoriAudioSource, IrodoriIdentity, IrodoriSettings};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilities {
    pub checkpoint: String,
    pub speaker_conditioning: bool,
    pub caption_conditioning: bool,
    pub sample_rate: u32,
    pub channels: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    None,
    Audio,
    Latent,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredVoice {
    pub id: String,
    pub name: String,
    pub reference: ReferenceKind,
    pub caption: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Catalog {
    model: ModelCapabilities,
    voices: BTreeMap<String, RegisteredVoice>,
}
#[derive(Debug, Clone)]
pub struct SelectedVoice {
    source: IrodoriAudioSource,
    reference: ReferenceKind,
    caption: Option<String>,
}
fn invalid(message: &str) -> InputError {
    InputError::Invalid(message.into())
}
fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}
fn caption(value: &Option<String>) -> bool {
    value
        .as_ref()
        .is_none_or(|v| !v.trim().is_empty() && v.len() <= 4096 && !v.contains('\0'))
}
impl Catalog {
    pub fn new(model: ModelCapabilities, voices: Vec<RegisteredVoice>) -> Result<Self, InputError> {
        if !text(&model.checkpoint, 4096)
            || !(8000..=192000).contains(&model.sample_rate)
            || !(1..=2).contains(&model.channels)
            || voices.is_empty()
            || voices.len() > 4096
        {
            return Err(invalid("Irodoriのモデル・話者情報が不正です"));
        }
        let mut entries = BTreeMap::new();
        for voice in voices {
            if !text(&voice.id, 128)
                || !text(&voice.name, 1024)
                || !caption(&voice.caption)
                || entries.insert(voice.id.clone(), voice).is_some()
            {
                return Err(invalid("Irodoriの話者情報が不正または重複しています"));
            }
        }
        Ok(Self {
            model,
            voices: entries,
        })
    }
    /// Metadata selection only. The owner must attest current model/voice originals.
    pub fn select(
        &self,
        settings: &IrodoriSettings,
        identity: &IrodoriIdentity,
    ) -> Result<SelectedVoice, InputError> {
        settings
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        identity
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        if self.model.checkpoint != identity.checkpoint
            || self.model.sample_rate != identity.sample_rate
            || self.model.channels != identity.channels
        {
            return Err(invalid("Irodoriのモデル情報と原本識別情報が一致しません"));
        }
        let voice = self
            .voices
            .get(&settings.voice_id)
            .ok_or_else(|| invalid("指定したIrodori話者がありません"))?;
        let caption = settings
            .caption
            .as_ref()
            .or(voice.caption.as_ref())
            .cloned();
        if voice.reference != ReferenceKind::None && !self.model.speaker_conditioning {
            return Err(invalid("このIrodoriモデルは参照音声に対応していません"));
        }
        if caption.is_some() && !self.model.caption_conditioning {
            return Err(invalid("このIrodoriモデルは声の説明文に対応していません"));
        }
        let bytes = serde_json::to_vec(&(
            "vvm-irodori-selection-v1",
            &self.model,
            &voice.id,
            voice.reference,
            &voice.caption,
        ))
        .map_err(|_| invalid("Irodori話者の情報を固定できません"))?;
        let source = IrodoriAudioSource {
            settings: settings.clone(),
            identity: identity.clone(),
            metadata_sha256: format!("{:x}", Sha256::digest(bytes)),
        };
        source
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        Ok(SelectedVoice {
            source,
            reference: voice.reference,
            caption,
        })
    }
    pub fn model(&self) -> &ModelCapabilities {
        &self.model
    }
    pub fn voices(&self) -> impl Iterator<Item = &RegisteredVoice> {
        self.voices.values()
    }
}
impl SelectedVoice {
    pub fn audio_source(&self) -> &IrodoriAudioSource {
        &self.source
    }
    pub fn reference(&self) -> ReferenceKind {
        self.reference
    }
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }
    pub fn no_reference(&self) -> bool {
        self.reference == ReferenceKind::None
    }
}
