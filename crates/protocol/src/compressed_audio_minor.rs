use sungeki_editor::Command;
use sungeki_project::{Asset, AssetProbe};
fn compressed(asset: &Asset) -> bool {
    matches!(&asset.probe, AssetProbe::Audio(probe) if probe.encoding.is_some())
}
pub(crate) fn command(command: &Command) -> bool {
    match command {
        Command::RegisterAsset { asset } => compressed(asset),
        Command::ReplaceProject { project, .. } => project.assets.iter().any(compressed),
        _ => false,
    }
}
