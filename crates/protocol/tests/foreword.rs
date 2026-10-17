use sungeki_editor::{Command, EditAction, EditRequest, ForewordTemplate};
use sungeki_project::TimeUs;
use sungeki_protocol::SessionRequest;
#[test]
fn requires_minor_56_for_preview_and_edit() {
    let commands = vec![Command::PrependForeword {
        template: ForewordTemplate {
            preset_id: "foreword-wide-v1".into(),
            background_asset_id: "a".into(),
            text: "前書き".into(),
            duration_us: TimeUs(8_000_000),
            scene_id: "s".into(),
            track_id: "t".into(),
            clip_ids: vec![],
        },
    }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 56);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 56);
}
