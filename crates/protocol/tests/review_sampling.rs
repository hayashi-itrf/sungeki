use sungeki_protocol::*;
#[test]
fn explicit_sampling_requires_capable_host_and_rejects_unknown_fields() {
    let value = serde_json::json!({"type":"start_review", "session_id":"session", "job_id":"job", "expected_revision":0,
        "sampling":{"requested_times_us":[3500000],"max_frames":8}});
    let request: SessionRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 75);
    let mut bad = value;
    bad["sampling"]["paths"] = serde_json::json!([]);
    assert!(serde_json::from_value::<SessionRequest>(bad).is_err());
    assert_eq!(ReviewFrameOptions::default().max_frames, 12);
}
