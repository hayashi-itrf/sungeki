use sungeki_protocol::*;
#[test]
fn speech_get_wait_defaults_and_is_versioned() {
    for wait in [None, Some(0), Some(30_000)] {
        let mut value =
            serde_json::json!({"type":"speech_job","request":{"type":"get","job_id":"j"}});
        if let Some(wait) = wait {
            value["request"]["wait_ms"] = wait.into();
        }
        let request: SessionRequest = serde_json::from_value(value).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 138);
        assert!(
            matches!(request, SessionRequest::SpeechJob { request: SpeechJobRequest::Get {wait_ms, ..} } if wait_ms == wait.unwrap_or(0))
        );
    }
    let request: SessionRequest = serde_json::from_value(
        serde_json::json!({"type":"speech_job","request":{"type":"cancel","job_id":"j"}}),
    )
    .unwrap();
    assert_eq!(request.minimum_protocol_minor(), 32);
}

#[test]
fn all_speech_starts_have_optional_response_wait() {
    for (kind, baseline) in [
        ("start", 32),
        ("start_sbv2", 68),
        ("start_qwen", 101),
        ("start_irodori", 105),
    ] {
        let mut args =
            serde_json::json!({"session_id":"s","job_id":"j","expected_revision":0,"clip_id":"c"});
        if kind == "start" {
            args["base_url"] = "http://127.0.0.1:1".into();
            args["engine_version"] = "1".into();
        } else {
            args["profile_id"] = "local".into();
        }
        let mut value =
            serde_json::json!({"type":"speech_job","request":{"type":kind,"args":args}});
        let parsed: SessionRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(parsed.minimum_protocol_minor(), baseline);
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        for wait in [0, 30000] {
            value["request"]["args"]["wait_ms"] = wait.into();
            let parsed: SessionRequest = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(parsed.minimum_protocol_minor(), 139);
            assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        }
        value["request"]["args"]["wait_ms"] = (-1).into();
        assert!(serde_json::from_value::<SessionRequest>(value).is_err());
    }
}
