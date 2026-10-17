use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_protocol::SessionRequest;
#[test]
fn resize_layout_requires_minor_131_for_preview_and_edit() {
    let commands = vec![Command::ResizeLayout {
        width: 1080,
        height: 1920,
    }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 131);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 131);
}
