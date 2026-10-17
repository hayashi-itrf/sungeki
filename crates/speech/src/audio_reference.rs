use crate::{InputError, PreparedSpeechClip, VoicevoxIdentity};
use sungeki_project::{ClipContent, Project, Voice};

/// Checks the saved synthesis input, independently of clip placement/style.
/// Does not open files or attest that an engine is currently available.
/// Consumers must separately verify the asset's original bytes before using it.
pub fn audio_reference_matches(project: &Project, clip_id: &str) -> Result<bool, InputError> {
    let clip = project
        .clips
        .iter()
        .find(|c| c.id == clip_id)
        .ok_or_else(|| InputError::Invalid("音声クリップが見つかりません".into()))?;
    let ClipContent::Speech {
        audio: Some(audio),
        character_id,
        ..
    } = &clip.content
    else {
        return Ok(false);
    };
    let character = project
        .characters
        .iter()
        .find(|c| c.id == *character_id)
        .ok_or_else(|| InputError::Invalid("キャラクターが見つかりません".into()))?;
    if matches!(character.voice, Voice::Irodori { .. }) {
        return crate::irodori::reference_matches(project, clip_id);
    }
    if matches!(character.voice, Voice::Qwen { .. }) {
        return crate::qwen::reference_matches(project, clip_id);
    }
    let Voice::Voicevox {
        engine_id,
        core_version,
        ..
    } = &character.voice
    else {
        return crate::sbv2::reference_matches(project, clip_id);
    };
    if audio.sbv2_source.is_some() || audio.qwen_source.is_some() || audio.irodori_source.is_some()
    {
        return Ok(false);
    }
    let identity = VoicevoxIdentity {
        engine_id: engine_id.clone(),
        core_version: core_version.clone(),
        engine_version: audio.engine_version.clone(),
    };
    let input = PreparedSpeechClip::new(project, clip_id, &identity)?;
    Ok(input.audio().fingerprint() == audio.input_fingerprint)
}
