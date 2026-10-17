use crate::{ResolvedText, TextError, resolve_text, voicevox::VoicevoxSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sungeki_project::PronunciationEntry;

/// Actual engine metadata discovered by the caller, never inferred from a URL.
/// This is input identity, not proof that a running endpoint still has that identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoicevoxIdentity {
    pub engine_id: String,
    pub engine_version: String,
    pub core_version: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    Text(TextError),
    Invalid(String),
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(error) => error.fmt(f),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for InputError {}

/// Immutable audio-input snapshot. This hash alone does NOT authorize applying a
/// job result: the host must also check clip identity/state and pose/lipsync input.
#[derive(Debug, Clone)]
pub struct PreparedVoicevoxInput {
    subtitle: String,
    spoken: Option<String>,
    dictionary: Vec<PronunciationEntry>,
    resolved: ResolvedText,
    settings: VoicevoxSettings,
    identity: VoicevoxIdentity,
    fingerprint: String,
    dictionary_fingerprint: String,
}
impl PreparedVoicevoxInput {
    pub fn new(
        subtitle: &str,
        spoken: Option<&str>,
        dictionary: &[PronunciationEntry],
        settings: &VoicevoxSettings,
        identity: &VoicevoxIdentity,
    ) -> Result<Self, InputError> {
        if subtitle.len() > crate::MAX_SPEECH_INPUT_BYTES || subtitle.contains('\0') {
            return Err(InputError::Invalid(
                "字幕が64KiBを超えるかNULを含んでいます".into(),
            ));
        }
        for value in [
            &identity.engine_id,
            &identity.engine_version,
            &identity.core_version,
        ] {
            if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
                return Err(InputError::Invalid(
                    "エンジン識別情報が空または不正です".into(),
                ));
            }
        }
        settings
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        let resolved = resolve_text(subtitle, spoken, dictionary).map_err(InputError::Text)?;
        let dictionary_fingerprint = hash(&("vvm-pronunciation-dictionary-v1", dictionary))?;
        // Include source text as well as the final reading: identical readings
        // must not conceal subtitle edits or an explicit-reading mode change.
        let fingerprint = hash(&(
            "vvm-voicevox-audio-input-v1",
            subtitle,
            spoken,
            &resolved.text,
            &dictionary_fingerprint,
            settings,
            identity,
        ))?;
        Ok(Self {
            subtitle: subtitle.into(),
            spoken: spoken.map(str::to_owned),
            dictionary: dictionary.to_vec(),
            resolved,
            settings: settings.clone(),
            identity: identity.clone(),
            fingerprint,
            dictionary_fingerprint,
        })
    }
    pub fn subtitle(&self) -> &str {
        &self.subtitle
    }
    pub fn spoken(&self) -> Option<&str> {
        self.spoken.as_deref()
    }
    pub fn dictionary(&self) -> &[PronunciationEntry] {
        &self.dictionary
    }
    pub fn resolved(&self) -> &ResolvedText {
        &self.resolved
    }
    pub fn settings(&self) -> &VoicevoxSettings {
        &self.settings
    }
    pub fn identity(&self) -> &VoicevoxIdentity {
        &self.identity
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn dictionary_fingerprint(&self) -> &str {
        &self.dictionary_fingerprint
    }
}
fn hash(value: &impl Serialize) -> Result<String, InputError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| InputError::Invalid("合成入力を固定できません".into()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
