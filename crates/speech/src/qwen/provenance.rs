use super::PreparedQwenInput;
use crate::{InputError, target::SpeechTarget};
use sungeki_project::{ClipContent, Project, QwenAudioSource, Voice};
impl PreparedQwenInput {
    pub fn audio_source(&self) -> Result<QwenAudioSource, InputError> {
        let source = QwenAudioSource {
            selection: self.selection().clone(),
            identity: self
                .identity()
                .cloned()
                .ok_or_else(|| InputError::Invalid("モデル識別情報がありません".into()))?,
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
    let Some(source) = &audio.qwen_source else {
        return Ok(false);
    };
    let Voice::Qwen { settings, identity } = &target.character.voice else {
        return Ok(false);
    };
    source
        .validate()
        .map_err(|e| InputError::Invalid(e.to_string()))?;
    if identity != &source.identity
        || settings.model != source.selection.request_model
        || settings.voice != source.selection.voice
        || settings.language != source.selection.language
        || audio.engine_version != identity.engine_manifest_sha256
    {
        return Ok(false);
    }
    let (subtitle, spoken) = target.text();
    let input = PreparedQwenInput::from_selection(
        subtitle,
        spoken,
        &project.pronunciation_dictionary,
        settings,
        source.selection.clone(),
        sungeki_media::ExpectedAudio {
            sample_rate: source.identity.sample_rate,
            channels: source.identity.channels,
        },
    )?
    .bind_identity(identity)
    .map_err(|e| InputError::Invalid(e.to_string()))?;
    Ok(input.fingerprint() == audio.input_fingerprint)
}
