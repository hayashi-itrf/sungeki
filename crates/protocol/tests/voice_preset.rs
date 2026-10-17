use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::{Preset, PresetContent, Project};
use sungeki_protocol::SessionRequest;
#[test]
fn voice_preset_requires_minor_130_for_preview_and_edit() {
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let commands = vec![Command::ApplyVoicePreset {
        preset_id: p.id.clone(),
        preset_revision: 1,
        preset: Preset {
            schema_version: 1,
            name: "voice".into(),
            description: String::new(),
            tags: vec![],
            content: PresetContent::Voice {
                voice: p.characters[0].voice.clone(),
                credit: p.characters[0].credit.clone(),
            },
        },
        character_id: p.characters[0].id.clone(),
        credit: p.characters[0].credit.clone(),
    }];
    let preview = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: commands.clone(),
        offset: 0,
        limit: 10,
    };
    assert_eq!(preview.minimum_protocol_minor(), 130);
    let edit = SessionRequest::Edit {
        session_id: "s".into(),
        edit: EditRequest {
            operation_id: "o".into(),
            expected_revision: 0,
            action: EditAction::Apply { commands },
        },
    };
    assert_eq!(edit.minimum_protocol_minor(), 130);
}
