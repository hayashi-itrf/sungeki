use crate::{InputError, resolve_text, target::SpeechTarget};
use sha2::{Digest, Sha256};
use sungeki_media::{ExpectedAudio, VerifiedWav, WavError, verify_wav};
use sungeki_project::{Character, LocalVoiceSettings, Project, Voice};
#[derive(Debug, Clone)]
pub struct PreparedLocalInput {
    target: SpeechTarget,
    settings: LocalVoiceSettings,
    text: String,
    version: String,
    fingerprint: String,
}
impl PreparedLocalInput {
    pub fn new(project: &Project, clip_id: &str, version: &str) -> Result<Self, InputError> {
        if version.is_empty() || version.len() > 128 || version.chars().any(char::is_control) {
            return Err(InputError::Invalid("エンジン版が不正です".into()));
        }
        let target = SpeechTarget::capture(project, clip_id)?;
        let Voice::LocalHttp { settings } = &target.character.voice else {
            return Err(InputError::Invalid(
                "ローカルHTTP音声を指定してください".into(),
            ));
        };
        let settings = settings.clone();
        let (subtitle, spoken) = target.text();
        let text = resolve_text(subtitle, spoken, &project.pronunciation_dictionary)
            .map_err(InputError::Text)?
            .text;
        if text.chars().count() > 4096 {
            return Err(InputError::Invalid("読み上げは4096文字以内です".into()));
        }
        let bytes = serde_json::to_vec(&(
            "sungeki-local-http-audio-v1",
            subtitle,
            spoken,
            &project.pronunciation_dictionary,
            &text,
            &settings,
            version,
            44100u32,
        ))
        .map_err(|e| InputError::Invalid(e.to_string()))?;
        Ok(Self {
            target,
            settings,
            text,
            version: version.into(),
            fingerprint: format!("{:x}", Sha256::digest(bytes)),
        })
    }
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    pub fn settings(&self) -> &LocalVoiceSettings {
        &self.settings
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn character(&self) -> &Character {
        &self.target.character
    }
    pub fn matches(&self, project: &Project) -> bool {
        project.id == self.target.project_id
            && Self::new(project, &self.target.clip.id, &self.version)
                .is_ok_and(|p| p.target == self.target && p.fingerprint == self.fingerprint)
    }
    pub fn verify_audio(&self, bytes: Vec<u8>) -> Result<VerifiedWav, WavError> {
        verify_wav(
            bytes,
            ExpectedAudio {
                sample_rate: 44100,
                channels: 1,
            },
        )
    }
}
