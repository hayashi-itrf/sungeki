use sungeki_protocol::*;
#[test]
fn audio_preview_wait_is_optional_and_versioned() {
    for kind in ["start", "get"] {
        let mut input = serde_json::json!({"type":"audio_preview", "request":{"type":kind}});
        if kind == "start" {
            input["request"]["args"] =
                serde_json::json!({"session_id":"s","job_id":"j","expected_revision":0});
        } else {
            input["request"]["job_id"] = "j".into();
        }
        let request: SessionRequest = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(
            request.minimum_protocol_minor(),
            if kind == "start" { 44 } else { 140 }
        );
        for ms in [0, 30_000] {
            if kind == "start" {
                input["request"]["args"]["wait_ms"] = ms.into();
            } else {
                input["request"]["wait_ms"] = ms.into();
            }
            let request: SessionRequest = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(request.minimum_protocol_minor(), 140);
            assert_eq!(serde_json::to_value(request).unwrap(), input);
        }
    }
    let schema = serde_json::to_value(schemars::schema_for!(StartAudioPreviewArgs)).unwrap();
    assert!(schema["properties"].get("wait_ms").is_some());
    assert!(
        serde_json::from_value::<AudioPreviewRequest>(
            serde_json::json!({"type":"cancel","job_id":"j","wait_ms":1})
        )
        .is_err()
    );
}
