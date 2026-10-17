use crate::Command;
use sungeki_project::{ClipContent, Project, ValidationError};

pub(crate) fn apply(project: &mut Project, command: &Command) -> Result<(), ValidationError> {
    match command {
        Command::SetSpeechGain { clip_id, gain_db } => {
            let clip = project
                .clips
                .iter_mut()
                .find(|c| c.id == *clip_id)
                .ok_or_else(|| ValidationError::new("音声クリップが見つかりません".into()))?;
            let ClipContent::Speech {
                gain_db: current, ..
            } = &mut clip.content
            else {
                return Err(ValidationError::new(
                    "音声クリップを指定してください".into(),
                ));
            };
            *current = *gain_db;
        }
        Command::SetSpeechAudio { clip_id, audio } => {
            let clip = project
                .clips
                .iter_mut()
                .find(|c| c.id == *clip_id)
                .ok_or_else(|| ValidationError::new("音声クリップが見つかりません".into()))?;
            let ClipContent::Speech { audio: current, .. } = &mut clip.content else {
                return Err(ValidationError::new(
                    "音声クリップを指定してください".into(),
                ));
            };
            *current = audio.clone();
        }
        Command::SetCharacter { character } => {
            if project
                .characters
                .iter()
                .any(|c| c.id == character.id && c.voice != character.voice)
            {
                clear_audio(project, |id| id == character.id);
            }
            if let Some(current) = project.characters.iter_mut().find(|c| c.id == character.id) {
                *current = character.clone();
            } else {
                project.characters.push(character.clone());
            }
        }
        Command::RemoveCharacter { character_id } => {
            if !project.characters.iter().any(|c| c.id == *character_id) {
                return Err(ValidationError::new("キャラクターが見つかりません".into()));
            }
            // Final shared validation rejects dangling references. A batch may
            // explicitly delete/reassign all uses and then remove the character.
            project.characters.retain(|c| c.id != *character_id);
        }
        Command::SetSpeech {
            clip_id,
            character_id,
            subtitle_text,
            spoken_text,
        } => {
            let clip = project
                .clips
                .iter_mut()
                .find(|c| c.id == *clip_id)
                .ok_or_else(|| ValidationError::new("音声クリップが見つかりません".into()))?;
            let ClipContent::Speech {
                character_id: old_character,
                subtitle_text: old_subtitle,
                spoken_text: old_spoken,
                audio,
                gain_db,
            } = &clip.content
            else {
                return Err(ValidationError::new(
                    "音声以外のクリップへ発話を設定できません".into(),
                ));
            };
            let retained = if old_character == character_id
                && old_subtitle == subtitle_text
                && old_spoken == spoken_text
            {
                audio.clone()
            } else {
                None
            };
            clip.content = ClipContent::Speech {
                character_id: character_id.clone(),
                subtitle_text: subtitle_text.clone(),
                spoken_text: spoken_text.clone(),
                audio: retained,
                gain_db: *gain_db,
            };
        }
        _ => unreachable!("speech command routing"),
    }
    Ok(())
}

// Retain the registered asset: history and checkpoints may still reference it.
pub(crate) fn clear_audio(project: &mut Project, affected: impl Fn(&str) -> bool) {
    for clip in &mut project.clips {
        if let ClipContent::Speech {
            character_id,
            audio,
            ..
        } = &mut clip.content
        {
            if affected(character_id) {
                *audio = None;
            }
        }
    }
}
