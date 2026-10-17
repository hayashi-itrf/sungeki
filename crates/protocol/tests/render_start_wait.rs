use sungeki_protocol::*;

#[test]
fn render_and_preview_start_wait_is_optional_and_versioned() {
    for (kind, baseline) in [("start", 29), ("preview", 111)] {
        let mut input = serde_json::json!({"type":"render_job", "request":{
            "type":kind, "args":{"session_id":"s", "job_id":"j", "expected_revision":0}
        }});
        let request: SessionRequest = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(request.minimum_protocol_minor(), baseline);
        assert!(
            serde_json::to_value(request).unwrap()["request"]["args"]
                .get("wait_ms")
                .is_none()
        );
        for wait in [0, 30_000] {
            input["request"]["args"]["wait_ms"] = wait.into();
            let request: SessionRequest = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(request.minimum_protocol_minor(), 137);
            assert_eq!(serde_json::to_value(request).unwrap(), input);
        }
        input["request"]["args"]["wait_ms"] = (-1).into();
        assert!(serde_json::from_value::<SessionRequest>(input).is_err());
    }
}
