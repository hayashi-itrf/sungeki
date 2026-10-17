use sungeki_editor::{Command, SpeechScript};
use sungeki_project::TimeUs;
use sungeki_protocol::SessionRequest;
#[test]
fn script_edit_requires_minor_45_even_in_preview() {
    let request = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: vec![Command::AppendSpeechScript {
            script: SpeechScript {
                lines: vec![],
                character_id: "c".into(),
                track_id: "t".into(),
                scene_id: None,
                start_us: TimeUs(0),
                provisional_duration_us: TimeUs(1),
                gap_us: TimeUs(0),
            },
        }],
        offset: 0,
        limit: 10,
    };
    assert_eq!(request.minimum_protocol_minor(), 45);
    let mut request = request;
    if let SessionRequest::PreviewEdit { commands, .. } = &mut request {
        if let Command::AppendSpeechScript { script } = &mut commands[0] {
            script.lines.push(sungeki_editor::SpeechScriptLine {
                character_id: Some("second".into()),
                clip_id: "c".into(),
                subtitle_text: "dialogue".into(),
                spoken_text: None,
            });
        }
    }
    assert_eq!(request.minimum_protocol_minor(), 49);
}
