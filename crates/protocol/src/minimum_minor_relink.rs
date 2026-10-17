use crate::SessionRequest;
use sungeki_editor::{Command, EditAction};
pub(super) fn required(request: &SessionRequest) -> bool {
    let commands = match request {
        SessionRequest::Edit { edit, .. } => match &edit.action {
            EditAction::Apply { commands } => commands,
            _ => return false,
        },
        SessionRequest::PreviewEdit { commands, .. } => commands,
        _ => return false,
    };
    commands
        .iter()
        .any(|command| matches!(command, Command::RelinkAsset { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tagged_relink_requires_new_protocol_even_for_images() {
        let project: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/sprite-project.json"))
                .unwrap();
        let asset = &project.assets[0];
        let commands = vec![Command::RelinkAsset {
            asset_id: asset.id.clone(),
            locator: asset.locator.clone(),
            probe: asset.probe.clone(),
        }];
        let request = SessionRequest::PreviewEdit {
            session_id: "test".into(),
            expected_revision: 0,
            commands: commands.clone(),
            offset: 0,
            limit: 10,
        };
        assert_eq!(request.minimum_protocol_minor(), 120);
        let request: SessionRequest = serde_json::from_value(serde_json::json!({
            "type": "edit", "session_id": "test", "edit": {
                "operation_id": "test", "expected_revision": 0,
                "action": { "type": "apply", "commands": commands }
            }
        }))
        .unwrap();
        assert_eq!(request.minimum_protocol_minor(), 120);
    }
}
