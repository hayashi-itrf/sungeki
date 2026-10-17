//! Shared immutable speech target for all engines. No job execution or Project edits.
use crate::InputError;
use sungeki_project::{Character, Clip, ClipContent, Project, validate};
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpeechTarget {
    pub project_id: String,
    pub clip: Clip,
    pub character: Character,
}
impl SpeechTarget {
    pub fn capture(project: &Project, clip_id: &str) -> Result<Self, InputError> {
        validate(project).map_err(|e| InputError::Invalid(e.to_string()))?;
        let clip = project
            .clips
            .iter()
            .find(|c| c.id == clip_id)
            .ok_or_else(|| InputError::Invalid("音声クリップが見つかりません".into()))?;
        let ClipContent::Speech { character_id, .. } = &clip.content else {
            return Err(InputError::Invalid("音声クリップを指定してください".into()));
        };
        let character = project
            .characters
            .iter()
            .find(|c| c.id == *character_id)
            .ok_or_else(|| InputError::Invalid("キャラクターが見つかりません".into()))?;
        Ok(Self {
            project_id: project.id.clone(),
            clip: clip.clone(),
            character: character.clone(),
        })
    }
    pub fn text(&self) -> (&str, Option<&str>) {
        let ClipContent::Speech {
            subtitle_text,
            spoken_text,
            ..
        } = &self.clip.content
        else {
            unreachable!()
        };
        (subtitle_text, spoken_text.as_deref())
    }
}
