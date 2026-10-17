use sungeki_editor::Command;
pub(crate) fn command(command: &Command) -> bool {
    match command {
        Command::SetTransform { transform, .. } => transform.motion.is_some(),
        Command::AddClip { clip } => clip.transform.motion.is_some(),
        Command::ReplaceProject { project, .. } => {
            project.clips.iter().any(|c| c.transform.motion.is_some())
        }
        _ => false,
    }
}
