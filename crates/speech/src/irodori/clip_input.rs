use super::{Catalog, PreparedIrodoriInput};
use crate::{InputError, target::SpeechTarget};
use sungeki_project::{Character, Clip, IrodoriIdentity, Project, Voice};
#[derive(Debug, Clone)]
pub struct PreparedIrodoriClip {
    target: SpeechTarget,
    audio: PreparedIrodoriInput,
}
impl PreparedIrodoriClip {
    /// The host inspects runtime originals before supplying observed identity.
    pub fn new(
        project: &Project,
        clip_id: &str,
        catalog: &Catalog,
        observed: &IrodoriIdentity,
    ) -> Result<Self, InputError> {
        let target = SpeechTarget::capture(project, clip_id)?;
        let Voice::Irodori { settings, identity } = &target.character.voice else {
            return Err(InputError::Invalid(
                "Irodoriの話者を指定してください".into(),
            ));
        };
        if identity != observed {
            return Err(InputError::Invalid(
                "話者のモデル識別情報と実行環境が一致しません".into(),
            ));
        }
        let (subtitle, spoken) = target.text();
        let selected = catalog.select(settings, observed)?;
        let audio = PreparedIrodoriInput::from_selected(
            subtitle,
            spoken,
            &project.pronunciation_dictionary,
            &selected,
        )?;
        Ok(Self { target, audio })
    }
    pub fn audio(&self) -> &PreparedIrodoriInput {
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
        observed: &IrodoriIdentity,
    ) -> bool {
        Self::new(project, &self.target.clip.id, catalog, observed).is_ok_and(|current| {
            current.target == self.target && current.audio.fingerprint() == self.audio.fingerprint()
        })
    }
}
