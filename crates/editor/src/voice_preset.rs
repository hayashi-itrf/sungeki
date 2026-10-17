use crate::{Command, apply_batch};
use sungeki_project::*;

/// Keep the character's name, subtitles and standing bindings. The caller
/// explicitly reviews the final attribution rather than silently losing it.
pub fn plan_voice_preset(
    project: &Project,
    preset: &Preset,
    character_id: &str,
    credit: &str,
) -> Result<Vec<Command>, ValidationError> {
    preset.validate()?;
    let PresetContent::Voice {
        voice,
        credit: required,
    } = &preset.content
    else {
        return Err(ValidationError::new(
            "声プリセットを指定してください".into(),
        ));
    };
    if !credit.contains(required) {
        return Err(ValidationError::new(
            "適用後のクレジットに声プリセットのクレジットを含めてください".into(),
        ));
    }
    let mut character = project
        .characters
        .iter()
        .find(|c| c.id == character_id)
        .cloned()
        .ok_or_else(|| ValidationError::new("適用先の人物が見つかりません".into()))?;
    character.voice = voice.clone();
    character.credit = credit.into();
    let commands = vec![Command::SetCharacter { character }];
    // The canonical SetCharacter path invalidates synthesized speech only when
    // the voice actually changed; Undo retains the previous audio and settings.
    apply_batch(project, &commands)?;
    Ok(commands)
}
