use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::TimeUs;
use sungeki_protocol::SessionRequest;
#[test]
fn layout_requires_minor_46_for_preview_and_edit() {
    let commands = vec![Command::LayoutSpeech {
        clip_ids: vec![],
        start_us: TimeUs(0),
        gap_us: TimeUs(0),
    }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 46);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 46);
}
