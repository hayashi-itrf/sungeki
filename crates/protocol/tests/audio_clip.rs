use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::{ClipContent, Project};
use sungeki_protocol::SessionRequest;
#[test]
fn audio_commands_require_minor_42_on_both_edit_paths() {
    let project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    let ClipContent::Audio { asset_id, playback } = project.clips[0].content.clone() else {
        panic!()
    };
    for command in [
        Command::AddClip {
            clip: project.clips[0].clone(),
        },
        Command::SetAudio {
            clip_id: project.clips[0].id.clone(),
            asset_id,
            playback,
        },
        Command::ReplaceProject {
            checkpoint_id: None,
            project,
        },
    ] {
        assert_eq!(
            SessionRequest::PreviewEdit {
                session_id: "s".into(),
                expected_revision: 0,
                commands: vec![command.clone()],
                offset: 0,
                limit: 10
            }
            .minimum_protocol_minor(),
            42
        );
        assert_eq!(
            SessionRequest::Edit {
                session_id: "s".into(),
                edit: EditRequest {
                    operation_id: "o".into(),
                    expected_revision: 0,
                    action: EditAction::Apply {
                        commands: vec![command]
                    }
                }
            }
            .minimum_protocol_minor(),
            42
        );
    }
}
