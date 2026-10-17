use super::{Catalog, PreparedQwenInput, RuntimeIdentity};
use crate::{InputError, target::SpeechTarget};
use sungeki_media::ExpectedAudio;
use sungeki_project::{Character, Clip, Project, Voice};
#[derive(Debug, Clone)]
pub struct PreparedQwenClip {
    target: SpeechTarget,
    audio: PreparedQwenInput,
}
impl PreparedQwenClip {
    /// The host inspects runtime originals before supplying observed identity.
    pub fn new(
        project: &Project,
        clip_id: &str,
        catalog: &Catalog,
        observed: &RuntimeIdentity,
    ) -> Result<Self, InputError> {
        let target = SpeechTarget::capture(project, clip_id)?;
        let Voice::Qwen { settings, identity } = &target.character.voice else {
            return Err(InputError::Invalid("Qwenの話者を指定してください".into()));
        };
        if identity != observed {
            return Err(InputError::Invalid(
                "話者のモデル識別情報と実行環境が一致しません".into(),
            ));
        }
        let (subtitle, spoken) = target.text();
        let audio = PreparedQwenInput::new(
            subtitle,
            spoken,
            &project.pronunciation_dictionary,
            settings,
            catalog,
            ExpectedAudio {
                sample_rate: identity.sample_rate,
                channels: identity.channels,
            },
        )?
        .bind_identity(observed)
        .map_err(|e| InputError::Invalid(e.to_string()))?;
        Ok(Self { target, audio })
    }
    pub fn audio(&self) -> &PreparedQwenInput {
        &self.audio
    }
    pub fn clip(&self) -> &Clip {
        &self.target.clip
    }
    pub fn character(&self) -> &Character {
        &self.target.character
    }
    /// Re-evaluate under the host apply lock to reject stale synthesis results.
    pub fn matches(
        &self,
        project: &Project,
        catalog: &Catalog,
        observed: &RuntimeIdentity,
    ) -> bool {
        Self::new(project, &self.target.clip.id, catalog, observed).is_ok_and(|current| {
            current.target == self.target && current.audio.fingerprint() == self.audio.fingerprint()
        })
    }
}
