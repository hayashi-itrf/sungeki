use super::PreparedIrodoriInput;
use crate::{InputError, target::SpeechTarget};
use sungeki_project::{ClipContent, Project, Voice};

/// Offline input comparison. Never attests the engine or original file bytes.
pub(crate) fn reference_matches(project: &Project, clip_id: &str) -> Result<bool, InputError> {
    let target = SpeechTarget::capture(project, clip_id)?;
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &target.clip.content
    else {
        return Ok(false);
    };
    let Some(source) = &audio.irodori_source else {
        return Ok(false);
    };
    let Voice::Irodori { settings, identity } = &target.character.voice else {
        return Ok(false);
    };
    if settings != &source.settings
        || identity != &source.identity
        || audio.engine_version != identity.engine_manifest_sha256
    {
        return Ok(false);
    }
    let (subtitle, spoken) = target.text();
    let input =
        PreparedIrodoriInput::new(subtitle, spoken, &project.pronunciation_dictionary, source)?;
    Ok(input.fingerprint() == audio.input_fingerprint)
}
