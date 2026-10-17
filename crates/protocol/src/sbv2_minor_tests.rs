use crate::*;
#[test]
fn sbv2_character_requires_minor_66() {
    let mut project: sungeki_project::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    let recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    project.characters.push(recipe.roles[0].character.clone());
    project.characters[0].voice=serde_json::from_value(serde_json::json!({"type":"sbv2",
        "settings":{"model_id":0,"speaker_id":0,"language":"JP","style":"Neutral","style_weight":1.,"length":1.,"sdp_ratio":0.2,"noise":0.6,"noisew":0.8,"auto_split":true,"split_interval":0.5},
        "identity":{"engine_revision":"test","model_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"style_vectors_sha256":"c".repeat(64),"text_encoder_fingerprint":"d".repeat(64)}})).unwrap();
    let request = SessionRequest::PreviewEdit {
        session_id: "test".into(),
        expected_revision: 0,
        offset: 0,
        limit: 10,
        commands: vec![sungeki_editor::Command::SetCharacter {
            character: project.characters[0].clone(),
        }],
    };
    assert_eq!(request.minimum_protocol_minor(), 66);
    let request = SessionRequest::PreviewEdit {
        session_id: "test".into(),
        expected_revision: 0,
        offset: 0,
        limit: 10,
        commands: vec![sungeki_editor::Command::ReplaceProject {
            checkpoint_id: None,
            project,
        }],
    };
    assert_eq!(request.minimum_protocol_minor(), 66);
}
