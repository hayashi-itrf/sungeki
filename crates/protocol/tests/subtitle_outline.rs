use sungeki_editor::Command;
use sungeki_project::{Character, SubtitleOutline};
use sungeki_protocol::SessionRequest;
#[test]
fn outlined_character_requires_minor_47() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let mut character: Character =
        serde_json::from_value(fixture["project"]["characters"][0].clone()).unwrap();
    character.subtitle_style.outline = Some(SubtitleOutline {
        width: 3,
        color: "#000000".into(),
    });
    let request = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: vec![Command::SetCharacter { character }],
        offset: 0,
        limit: 10,
    };
    assert_eq!(request.minimum_protocol_minor(), 47);
    let mut request = request;
    if let SessionRequest::PreviewEdit { commands, .. } = &mut request {
        if let Command::SetCharacter { character } = &mut commands[0] {
            character.subtitle_style.layout = Some(sungeki_project::SubtitleLayout {
                alignment: sungeki_project::TextAlignment::Center,
                wrap: true,
            });
        }
    }
    assert_eq!(request.minimum_protocol_minor(), 48);
}
