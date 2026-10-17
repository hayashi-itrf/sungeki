//! Immutable synthesis intent. Host must additionally bind and verify model originals.
use super::{Catalog, Selection, Settings};
use crate::{InputError, ResolvedText, resolve_text};
use sha2::{Digest, Sha256};
use sungeki_media::{ExpectedAudio, VerifiedWav, WavError, verify_wav};
use sungeki_project::PronunciationEntry;
#[derive(Debug, Clone)]
pub struct PreparedQwenInput {
    subtitle: String,
    spoken: Option<String>,
    dictionary: Vec<PronunciationEntry>,
    resolved: ResolvedText,
    settings: Settings,
    pub(super) selection: Selection,
    pub(super) expected: ExpectedAudio,
    pub(super) fingerprint: String,
    pub(super) identity: Option<super::RuntimeIdentity>,
}
impl PreparedQwenInput {
    /// Expected audio format comes from the owned engine configuration, not its output WAV.
    pub fn new(
        subtitle: &str,
        spoken: Option<&str>,
        dictionary: &[PronunciationEntry],
        settings: &Settings,
        catalog: &Catalog,
        expected: ExpectedAudio,
    ) -> Result<Self, InputError> {
        let selection = catalog
            .select(settings)
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        Self::from_selection(subtitle, spoken, dictionary, settings, selection, expected)
    }
    pub(super) fn from_selection(
        subtitle: &str,
        spoken: Option<&str>,
        dictionary: &[PronunciationEntry],
        settings: &Settings,
        selection: Selection,
        expected: ExpectedAudio,
    ) -> Result<Self, InputError> {
        settings
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        if subtitle.len() > crate::MAX_SPEECH_INPUT_BYTES
            || subtitle.contains('\0')
            || !(8000..=192000).contains(&expected.sample_rate)
            || !(1..=2).contains(&expected.channels)
        {
            return Err(InputError::Invalid(
                "Qwenの字幕または音声形式が不正です".into(),
            ));
        }
        let resolved = resolve_text(subtitle, spoken, dictionary).map_err(InputError::Text)?;
        if resolved.text.chars().count() > 4096 {
            return Err(InputError::Invalid(
                "Qwenの読み上げは4096文字以内にしてください".into(),
            ));
        }
        let bytes = serde_json::to_vec(&(
            "vvm-qwen-prepared-input-v1",
            subtitle,
            spoken,
            dictionary,
            &resolved.text,
            settings,
            &selection,
            expected.sample_rate,
            expected.channels,
        ))
        .map_err(|_| InputError::Invalid("Qwen合成入力を固定できません".into()))?;
        Ok(Self {
            subtitle: subtitle.into(),
            spoken: spoken.map(str::to_owned),
            dictionary: dictionary.to_vec(),
            resolved,
            settings: settings.clone(),
            selection,
            expected,
            identity: None,
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
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    pub fn selection(&self) -> &Selection {
        &self.selection
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// Validate on a blocking media worker before publishing/applying any result.
    pub fn verify_audio(&self, bytes: Vec<u8>) -> Result<VerifiedWav, WavError> {
        verify_wav(bytes, self.expected)
    }
}
