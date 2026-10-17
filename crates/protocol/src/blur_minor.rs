use sungeki_editor::Command;
use sungeki_project::{Clip, ClipContent};
pub(crate) fn command(command: &Command) -> bool {
    let blur = |clip: &Clip| matches!(clip.content, ClipContent::Blur { .. });
    match command {
        Command::SetBlur { .. } => true,
        Command::AddClip { clip } => blur(clip),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(blur),
        _ => false,
    }
}
