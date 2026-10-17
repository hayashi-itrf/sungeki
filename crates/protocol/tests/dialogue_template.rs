use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_protocol::SessionRequest;
#[test]
fn dialogue_requires_minor_50_for_preview_and_edit() {
    let template = serde_json::from_value(serde_json::json!({
        "preset_id": "dialogue-wide-v1", "title": "test",
        "speech_clip_ids": [], "participants": [],
        "background_clip_id": "background", "bgm_clip_id": "bgm",
        "start_us": 0, "gap_us": 0, "scene_id": "scene", "track_ids": [],
        "title_backdrop_id": "title-background", "credits_backdrop_id": "credits-background",
        "title_clip_id": "title", "credits_clip_id": "credits"
    }))
    .unwrap();
    let commands = vec![Command::ApplyDialogueTemplate { template }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 50);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 50);
}
