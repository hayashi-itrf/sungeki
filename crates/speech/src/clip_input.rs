use crate::target::SpeechTarget;
use crate::{InputError, PreparedVoicevoxInput, VoicevoxIdentity};
use sungeki_project::{Character, Clip, Project, Voice};

/// Fixed target plus audio input. No Project mutation or automatic job-result
/// application. Future pose/lipsync fields must join this captured target model.
#[derive(Debug, Clone)]
pub struct PreparedSpeechClip {
    target: SpeechTarget,
    audio: PreparedVoicevoxInput,
}
impl PreparedSpeechClip {
    pub fn new(
        project: &Project,
        clip_id: &str,
        identity: &VoicevoxIdentity,
    ) -> Result<Self, InputError> {
        let target = SpeechTarget::capture(project, clip_id)?;
        let (subtitle_text, spoken_text) = target.text();
        let Voice::Voicevox {
            engine_id,
            core_version,
            settings,
        } = &target.character.voice
        else {
            return Err(InputError::Invalid(
                "SBV2の合成ジョブはまだ接続されていません".into(),
            ));
        };
        if *engine_id != identity.engine_id || *core_version != identity.core_version {
            return Err(InputError::Invalid(
                "キャラクター設定と実エンジンの識別が一致しません".into(),
            ));
        }
        let audio = PreparedVoicevoxInput::new(
            subtitle_text,
            spoken_text,
            &project.pronunciation_dictionary,
            settings,
            identity,
        )?;
        Ok(Self { target, audio })
    }
    pub fn audio(&self) -> &PreparedVoicevoxInput {
        &self.audio
    }
    pub fn clip(&self) -> &Clip {
        &self.target.clip
    }
    pub fn character(&self) -> &Character {
        &self.target.character
    }
    /// Check against a CURRENT validated snapshot under the host's apply lock.
    /// The caller must still atomically validate/apply actual duration and scenes.
    /// Engine identity must be freshly verified by the lease owner.
    pub fn matches(&self, project: &Project, identity: &VoicevoxIdentity) -> bool {
        if self.target.project_id != project.id {
            return false;
        }
        Self::new(project, &self.target.clip.id, identity).is_ok_and(|current| {
            current.target == self.target && current.audio.fingerprint() == self.audio.fingerprint()
        })
    }
}
