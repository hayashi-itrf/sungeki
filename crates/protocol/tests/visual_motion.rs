use sungeki_editor::Command;
use sungeki_protocol::SessionRequest;
#[test]
fn motion_transform_requires_minor_58() {
    let transform=serde_json::from_value(serde_json::json!({"x":0,"y":0,"width":100,"height":100,"opacity":1,"motion":{"end_x":100,"end_y":20,"end_scale":2,"easing":"ease_in_out"}})).unwrap();
    let request = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: vec![Command::SetTransform {
            clip_id: "c".into(),
            transform,
        }],
        offset: 0,
        limit: 10,
    };
    assert_eq!(request.minimum_protocol_minor(), 58);
}
