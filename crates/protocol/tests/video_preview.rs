use sungeki_protocol::SessionRequest;
#[test]
fn video_preview_requires_the_new_host_and_rejects_extra_fields() {
    let request = serde_json::json!({"type":"video_preview","args":{
        "session_id":"session","expected_revision":7,"clip_id":"clip","time_us":123456}});
    let parsed: SessionRequest = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(parsed.minimum_protocol_minor(), 85);
    assert_eq!(serde_json::to_value(parsed).unwrap(), request);
    let mut extra = request;
    extra["args"]["source_path"] = "/unregistered.mov".into();
    assert!(serde_json::from_value::<SessionRequest>(extra).is_err());
}
