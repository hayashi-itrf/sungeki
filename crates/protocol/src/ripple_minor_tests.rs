use crate::*;
use sungeki_editor::Command;
use sungeki_project::TimeUs;
#[test]
fn ripple_requires_134_for_edit_and_preview_even_with_batch_references() {
    let mut commands = vec![Command::Ripple {
        clip_ids: vec!["$line".into()],
        boundary_us: TimeUs(1),
        delta_us: 2,
    }];
    commands.insert(
        0,
        Command::AddTrack {
            track: sungeki_project::Track {
                id: "$track".into(),
                name: "Track".into(),
                visible: true,
                muted: false,
                kind: sungeki_project::TrackKind::Visual,
            },
        },
    );
    let edit = SessionRequest::Edit {
        session_id: "session".into(),
        edit: EditRequest {
            operation_id: "operation".into(),
            expected_revision: 0,
            action: EditAction::Apply {
                commands: commands.clone(),
            },
        },
    };
    let preview = SessionRequest::PreviewEdit {
        session_id: "session".into(),
        expected_revision: 0,
        commands,
        offset: 0,
        limit: 10,
    };
    assert_eq!(edit.minimum_protocol_minor(), 134);
    assert_eq!(preview.minimum_protocol_minor(), 134);
}
