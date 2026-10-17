use sungeki_editor::Command;
use sungeki_protocol::SessionRequest;
#[test]
fn fade_transform_requires_minor_51() {
    let transform = serde_json::from_value(serde_json::json!({"x":0,"y":0,"width":100,"height":100,"opacity":1,"fade":{"in_us":100000,"out_us":0}})).unwrap();
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
    assert_eq!(request.minimum_protocol_minor(), 51);
}

#[test]
fn image_flip_requires_minor_52() {
    let clip: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut clip: sungeki_project::Clip = serde_json::from_value(clip["clips"][0].clone()).unwrap();
    clip.transform.flip_x = Some(true);
    let request = SessionRequest::PreviewEdit {
        session_id: "s".into(),
        expected_revision: 0,
        commands: vec![Command::AddClip { clip }],
        offset: 0,
        limit: 10,
    };
    assert_eq!(request.minimum_protocol_minor(), 52);
}
