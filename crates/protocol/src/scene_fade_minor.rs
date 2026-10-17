use sungeki_editor::Command;
pub(crate) fn command(command: &Command) -> bool {
    match command {
        Command::SetSceneFade { .. } => true,
        Command::AddScene { scene } => scene.fade.is_some(),
        Command::ReplaceProject { project, .. } => project.scenes.iter().any(|s| s.fade.is_some()),
        _ => false,
    }
}
