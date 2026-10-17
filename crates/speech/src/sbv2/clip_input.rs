use super::{Catalog, ModelConfig, PreparedSbv2Input};
use crate::{InputError, target::SpeechTarget};
use sungeki_project::{Character, Clip, Project, Sbv2Identity, Voice};
#[derive(Debug, Clone)]
pub struct PreparedSbv2Clip {
    target: SpeechTarget,
    audio: PreparedSbv2Input,
}
impl PreparedSbv2Clip {
    /// `observed` must be inspected by the host runtime owner. This constructor does no I/O.
    pub fn new(
        project: &Project,
        clip_id: &str,
        catalog: &Catalog,
        config: &ModelConfig,
        observed: &Sbv2Identity,
    ) -> Result<Self, InputError> {
        let target = SpeechTarget::capture(project, clip_id)?;
        let Voice::Sbv2 { settings, identity } = &target.character.voice else {
            return Err(InputError::Invalid("SBV2の話者を指定してください".into()));
        };
        if observed != identity {
            return Err(InputError::Invalid(
                "話者に保存されたモデル識別情報と実行環境が一致しません".into(),
            ));
        }
        let (subtitle, spoken) = target.text();
        let audio = PreparedSbv2Input::from_model_config(
            subtitle,
            spoken,
            &project.pronunciation_dictionary,
            settings,
            catalog,
            config,
        )?
        .bind_identity(observed)?;
        Ok(Self { target, audio })
    }
    pub fn audio(&self) -> &PreparedSbv2Input {
        &self.audio
    }
    pub fn clip(&self) -> &Clip {
        &self.target.clip
    }
    pub fn character(&self) -> &Character {
        &self.target.character
    }
    /// Check current target under the host apply lock; caller still applies duration atomically.
    pub fn matches(
        &self,
        project: &Project,
        catalog: &Catalog,
        config: &ModelConfig,
        observed: &Sbv2Identity,
    ) -> bool {
        Self::new(project, &self.target.clip.id, catalog, config, observed).is_ok_and(|current| {
            current.target == self.target && current.audio.fingerprint() == self.audio.fingerprint()
        })
    }
}
