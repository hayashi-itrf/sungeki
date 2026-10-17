use super::PreparedSbv2Input;
use crate::{InputError, target::SpeechTarget};
use sungeki_project::{ClipContent, Project, Sbv2AudioSource, Voice};
impl PreparedSbv2Input {
    pub fn audio_source(&self) -> Result<Sbv2AudioSource, InputError> {
        let source = Sbv2AudioSource {
            selection: self.selection().clone(),
            identity: self
                .identity()
                .cloned()
                .ok_or_else(|| InputError::Invalid("モデル識別情報がありません".into()))?,
            sample_rate: self.expected_audio().sample_rate,
            channels: self.expected_audio().channels,
        };
        source
            .validate()
            .map_err(|e| InputError::Invalid(e.to_string()))?;
        Ok(source)
    }
}
/// Offline fingerprint check only. Original asset bytes require separate verification.
pub(crate) fn reference_matches(project: &Project, clip_id: &str) -> Result<bool, InputError> {
    let target = SpeechTarget::capture(project, clip_id)?;
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &target.clip.content
    else {
        return Ok(false);
    };
    let Some(source) = &audio.sbv2_source else {
        return Ok(false);
    };
    let Voice::Sbv2 { settings, identity } = &target.character.voice else {
        return Ok(false);
    };
    source
        .validate()
        .map_err(|e| InputError::Invalid(e.to_string()))?;
    if identity != &source.identity
        || settings.model_id != source.selection.model_id
        || settings.speaker_id != source.selection.speaker_id
        || settings.style != source.selection.style
        || audio.engine_version != identity.engine_revision
    {
        return Ok(false);
    }
    let (subtitle, spoken) = target.text();
    let input = PreparedSbv2Input::from_selection(
        subtitle,
        spoken,
        &project.pronunciation_dictionary,
        settings,
        source.selection.clone(),
        sungeki_media::ExpectedAudio {
            sample_rate: source.sample_rate,
            channels: source.channels,
        },
    )?
    .with_config_hash(&identity.config_sha256)?
    .bind_identity(identity)?;
    Ok(input.fingerprint() == audio.input_fingerprint)
}
