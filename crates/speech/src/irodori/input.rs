use crate::{InputError, ResolvedText, resolve_text};
use sha2::{Digest, Sha256};
use sungeki_media::{ExpectedAudio, VerifiedWav, WavError, verify_wav};
use sungeki_project::{IrodoriAudioSource, PronunciationEntry};

/// Owns all input values. The host must independently inspect model/voice originals
/// and validate the selected model's capabilities before dispatch.
#[derive(Debug, Clone)]
pub struct PreparedIrodoriInput {
    subtitle: String,
    spoken: Option<String>,
    dictionary: Vec<PronunciationEntry>,
    resolved: ResolvedText,
    source: IrodoriAudioSource,
    selection: Option<super::SelectedVoice>,
    fingerprint: String,
}
impl PreparedIrodoriInput {
    /// Validates a currently selected model/voice before capturing synthesis intent.
    pub fn from_selected(
        subtitle: &str,
        spoken: Option<&str>,
        dictionary: &[PronunciationEntry],
        selected: &super::SelectedVoice,
    ) -> Result<Self, InputError> {
        let mut input = Self::new(subtitle, spoken, dictionary, selected.audio_source())?;
        input.selection = Some(selected.clone());
        Ok(input)
    }
    pub fn new(
        subtitle: &str,
        spoken: Option<&str>,
        dictionary: &[PronunciationEntry],
        source: &IrodoriAudioSource,
    ) -> Result<Self, InputError> {
        source
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        if subtitle.len() > crate::MAX_SPEECH_INPUT_BYTES || subtitle.contains('\0') {
            return Err(InputError::Invalid("Irodoriの字幕が不正です".into()));
        }
        let resolved = resolve_text(subtitle, spoken, dictionary).map_err(InputError::Text)?;
        // Sungeki request limit, not proof of a checkpoint's token/duration capacity.
        if resolved.text.chars().count() > 4096 {
            return Err(InputError::Invalid(
                "Irodoriの読み上げは4096文字以内です".into(),
            ));
        }
        let bytes = serde_json::to_vec(&(
            "vvm-irodori-prepared-input-v1",
            subtitle,
            spoken,
            dictionary,
            &resolved.text,
            source,
        ))
        .map_err(|_| InputError::Invalid("Irodori合成入力を固定できません".into()))?;
        Ok(Self {
            subtitle: subtitle.into(),
            spoken: spoken.map(str::to_owned),
            dictionary: dictionary.to_vec(),
            resolved,
            source: source.clone(),
            selection: None,
            fingerprint: format!("{:x}", Sha256::digest(bytes)),
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
    pub fn audio_source(&self) -> &IrodoriAudioSource {
        &self.source
    }
    /// None means offline provenance reconstruction, not dispatch-ready selection.
    pub fn selection(&self) -> Option<&super::SelectedVoice> {
        self.selection.as_ref()
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// WAV headers cannot override the captured output format.
    pub fn verify_audio(&self, bytes: Vec<u8>) -> Result<VerifiedWav, WavError> {
        verify_wav(
            bytes,
            ExpectedAudio {
                sample_rate: self.source.identity.sample_rate,
                channels: self.source.identity.channels,
            },
        )
    }
}
