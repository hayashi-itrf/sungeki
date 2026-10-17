use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_protocol::SessionRequest;
#[test]
fn visual_timing_requires_minor_53_for_preview_and_edit() {
    let commands = vec![Command::FitVisualToSpeech {
        clip_id: "v".into(),
        first_speech_id: "s1".into(),
        last_speech_id: "s2".into(),
    }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 53);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 53);
}
