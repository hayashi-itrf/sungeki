use sungeki_editor::Command;
use sungeki_project::{Clip, ClipContent};
fn clip(c: &Clip) -> bool {
    matches!(&c.content, ClipContent::Audio { playback, .. } if !playback.volume_points.is_empty())
}
pub(crate) fn command(c: &Command) -> bool {
    match c {
        Command::SetAudio { playback, .. } => !playback.volume_points.is_empty(),
        Command::AddClip { clip: c } => clip(c),
        Command::ReplaceProject { project, .. } => project.clips.iter().any(clip),
        _ => false,
    }
}
