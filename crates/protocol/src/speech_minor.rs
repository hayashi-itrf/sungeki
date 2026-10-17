pub(super) fn speech_command(command: &sungeki_editor::Command) -> bool {
    use sungeki_editor::Command;
    use sungeki_project::ClipContent;
    match command {
        Command::SetSpeechAudio { .. }
        | Command::SetCharacter { .. }
        | Command::RemoveCharacter { .. }
        | Command::SetSpeech { .. } => true,
        Command::AddClip { clip } => matches!(clip.content, ClipContent::Speech { .. }),
        Command::ReplaceProject { project, .. } => {
            !project.characters.is_empty()
                || project
                    .clips
                    .iter()
                    .any(|c| matches!(c.content, ClipContent::Speech { .. }))
        }
        _ => false,
    }
}
