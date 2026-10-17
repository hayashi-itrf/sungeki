//! Normalize early Sungeki image metadata inside recovery journals only.
//! Public Project/Command deserialization still requires the current format.
use serde_json::Value;
use sungeki_project::ImageProbe;

fn probe(value: &mut Value) -> Option<bool> {
    if value.get("type").is_some() {
        return Some(false);
    }
    serde_json::from_value::<ImageProbe>(value.clone()).ok()?;
    *value = serde_json::json!({"type": "image", "info": value.take()});
    Some(true)
}
fn project(value: &mut Value) -> Option<bool> {
    let mut changed = false;
    for asset in value.get_mut("assets")?.as_array_mut()? {
        changed |= probe(asset.get_mut("probe")?)?;
    }
    Some(changed)
}
pub(crate) fn upgrade(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut snapshot: Value = serde_json::from_slice(bytes).ok()?;
    let history = snapshot.get_mut("history")?;
    let mut changed = project(history.get_mut("project")?)?;
    for key in ["undo", "redo"] {
        for entry in history.get_mut(key)?.as_array_mut()? {
            changed |= project(entry.get_mut("project")?)?;
        }
    }
    for record in snapshot.get_mut("receipts")?.as_object_mut()?.values_mut() {
        let action = record.pointer_mut("/request/action")?;
        if action.get("type")?.as_str()? != "apply" {
            continue;
        }
        for command in action.get_mut("commands")?.as_array_mut()? {
            match command.get("type")?.as_str()? {
                "relink_asset" => changed |= probe(command.get_mut("probe")?)?,
                "register_asset" => changed |= probe(command.pointer_mut("/asset/probe")?)?,
                "replace_project" => changed |= project(command.get_mut("project")?)?,
                _ => {}
            }
        }
    }
    changed
        .then(|| serde_json::to_vec(&snapshot).ok())
        .flatten()
}
