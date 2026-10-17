use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_protocol::SessionRequest;
#[test]
fn batch_references_require_minor_132_for_preview_and_edit() {
    let commands = vec![Command::AddTrack {
        track: sungeki_project::Track {
            id: "$track".into(),
            name: "test".into(),
            kind: sungeki_project::TrackKind::Visual,
            visible: true,
            muted: false,
        },
    }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 132);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 132);
}
