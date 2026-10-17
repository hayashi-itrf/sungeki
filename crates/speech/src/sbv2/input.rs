//! Immutable requested input, not authorization to apply a result to Project.
use super::{Catalog, Error, ModelConfig, Selection, Settings};
use crate::{InputError, ResolvedText, resolve_text};
use sha2::{Digest, Sha256};
use sungeki_media::{ExpectedAudio, VerifiedWav, WavError, verify_wav};
use sungeki_project::{PronunciationEntry, Sbv2Identity};

#[derive(Debug, Clone)]
pub struct PreparedSbv2Input {
    subtitle: String,
    spoken: Option<String>,
    dictionary: Vec<PronunciationEntry>,
    resolved: ResolvedText,
    settings: Settings,
    selection: Selection,
    expected: ExpectedAudio,
    fingerprint: String,
    model_config_sha256: Option<String>,
    identity: Option<Sbv2Identity>,
}
impl PreparedSbv2Input {
    /// Expected format must come from the selected model configuration, not the response WAV.
    /// Catalog metadata does not prove model-file identity or engine revision.
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
        if subtitle.len() > crate::MAX_SPEECH_INPUT_BYTES
            || subtitle.contains('\0')
            || !(8000..=192000).contains(&expected.sample_rate)
            || !(1..=2).contains(&expected.channels)
        {
            return Err(InputError::Invalid(
                "SBV2の字幕または期待する音声形式が不正です".into(),
            ));
        }
        let resolved = resolve_text(subtitle, spoken, dictionary).map_err(InputError::Text)?;
        let bytes = serde_json::to_vec(&(
            "vvm-sbv2-prepared-input-v1",
            subtitle,
            spoken,
            dictionary,
            &resolved.text,
            settings,
            &selection,
            expected.sample_rate,
            expected.channels,
        ))
        .map_err(|_| InputError::Invalid("SBV2合成入力を固定できません".into()))?;
        Ok(Self {
            subtitle: subtitle.into(),
            spoken: spoken.map(str::to_owned),
            dictionary: dictionary.to_vec(),
            resolved,
            settings: settings.clone(),
            selection,
            expected,
            model_config_sha256: None,
            identity: None,
            fingerprint: format!("{:x}", Sha256::digest(bytes)),
        })
    }
    pub fn from_model_config(
        subtitle: &str,
        spoken: Option<&str>,
        dictionary: &[PronunciationEntry],
        settings: &Settings,
        catalog: &Catalog,
        config: &ModelConfig,
    ) -> Result<Self, InputError> {
        let expected = config
            .expected_audio(catalog, settings)
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        Self::new(subtitle, spoken, dictionary, settings, catalog, expected)?
            .with_config_hash(config.sha256())
    }
    pub(super) fn with_config_hash(mut self, hash: &str) -> Result<Self, InputError> {
        let bytes = serde_json::to_vec(&("vvm-sbv2-configured-input-v1", &self.fingerprint, hash))
            .map_err(|_| InputError::Invalid("model config fingerprint".into()))?;
        self.fingerprint = format!("{:x}", Sha256::digest(bytes));
        self.model_config_sha256 = Some(hash.into());
        Ok(self)
    }
    pub(super) fn expected_audio(&self) -> ExpectedAudio {
        self.expected
    }
    /// Bind declared runtime identity after config parsing; host must still verify original files.
    pub fn bind_identity(mut self, identity: &Sbv2Identity) -> Result<Self, InputError> {
        identity
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        if self.model_config_sha256.as_deref() != Some(identity.config_sha256.as_str())
            || self.identity.is_some()
        {
            return Err(InputError::Invalid(
                "SBV2の設定原本が一致しないか、識別情報が固定済みです".into(),
            ));
        }
        let bytes =
            serde_json::to_vec(&("vvm-sbv2-identified-input-v1", &self.fingerprint, identity))
                .map_err(|_| InputError::Invalid("SBV2原本識別情報を固定できません".into()))?;
        self.fingerprint = format!("{:x}", Sha256::digest(bytes));
        self.identity = Some(identity.clone());
        Ok(self)
    }
    pub fn identity(&self) -> Option<&Sbv2Identity> {
        self.identity.as_ref()
    }
    pub fn model_config_sha256(&self) -> Option<&str> {
        self.model_config_sha256.as_deref()
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
    /// Perform on a blocking/media worker. No persistence, Project edit or quality claim.
    pub fn verify_audio(&self, bytes: Vec<u8>) -> Result<VerifiedWav, WavError> {
        verify_wav(bytes, self.expected)
    }
}
impl super::Sbv2Client {
    /// Synthesizes the immutable resolved reading. Returned bytes still require verify_audio.
    pub async fn synthesize_prepared(
        &self,
        input: &PreparedSbv2Input,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<u8>, Error> {
        self.exchange_prepared(input, cancel)
            .await
            .map_err(|failure| failure.error)
    }
}
