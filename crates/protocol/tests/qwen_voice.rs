use serde_json::json;
use sungeki_protocol::SessionRequest;
#[test]
fn qwen_character_preview_requires_minor_99() {
    let recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let mut character = recipe["roles"][0]["character"].clone();
    character["voice"] = json!({"type":"qwen","settings":{"model":"qwen3-tts","voice":"Vivian","speed":1.,"language":"Japanese","instruct":null},"identity":{"backend":"official","model_id":"/ssd/qwen","model_manifest_sha256":"a".repeat(64),"engine_manifest_sha256":"b".repeat(64),"voice_manifest_sha256":"c".repeat(64),"sample_rate":24000,"channels":1}});
    let request:SessionRequest=serde_json::from_value(json!({"type":"preview_edit","session_id":"s","expected_revision":0,"offset":0,"limit":10,"commands":[{"type":"set_character","character":character}]})).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 99);
    let encoded = serde_json::to_value(&request).unwrap();
    assert_eq!(encoded["commands"][0]["character"]["voice"]["type"], "qwen");
}
#[test]
fn qwen_audio_requires_minor_100_even_without_character_edit() {
    let audio = json!({"asset_id":"a","input_fingerprint":"a".repeat(64),"engine_version":"b".repeat(64),"lip_sync":null,"qwen_source":{"selection":{"backend":"official","model_id":"model","request_model":"qwen3-tts","voice":"Vivian","language":"Japanese","metadata_sha256":"d".repeat(64)},"identity":{"backend":"official","model_id":"model","model_manifest_sha256":"a".repeat(64),"engine_manifest_sha256":"b".repeat(64),"voice_manifest_sha256":"c".repeat(64),"sample_rate":24000,"channels":1}}});
    let mut project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut clip = project["clips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["content"]["type"] == "speech")
        .unwrap()
        .clone();
    clip["content"]["audio"] = audio.clone();
    project["clips"] = json!([clip.clone()]);
    for command in [
        json!({"type":"set_speech_audio","clip_id":"c","audio":audio}),
        json!({"type":"add_clip","clip":clip}),
        json!({"type":"replace_project","project":project,"checkpoint_id":null}),
    ] {
        let request:SessionRequest=serde_json::from_value(json!({"type":"preview_edit","session_id":"s","expected_revision":0,"offset":0,"limit":10,"commands":[command]})).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 100);
    }
}
