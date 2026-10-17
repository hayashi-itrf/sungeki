use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::{ClipContent, Project};
use sungeki_protocol::SessionRequest;
#[test]
fn blur_requires_minor_55_in_all_content_entry_points() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let mut project: Project = serde_json::from_value(value["project"].clone()).unwrap();
    project.clips[0].content = ClipContent::Blur { sigma: 18. };
    for command in [
        Command::SetBlur {
            clip_id: "c".into(),
            sigma: 18.,
        },
        Command::AddClip {
            clip: project.clips[0].clone(),
        },
        Command::ReplaceProject {
            checkpoint_id: None,
            project,
        },
    ] {
        let commands = vec![command];
        let preview = SessionRequest::PreviewEdit {
            session_id: "s".into(),
            expected_revision: 0,
            commands: commands.clone(),
            offset: 0,
            limit: 10,
        };
        assert_eq!(preview.minimum_protocol_minor(), 55);
        let edit = SessionRequest::Edit {
            session_id: "s".into(),
            edit: EditRequest {
                operation_id: "o".into(),
                expected_revision: 0,
                action: EditAction::Apply { commands },
            },
        };
        assert_eq!(edit.minimum_protocol_minor(), 55);
    }
}
