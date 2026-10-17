//! Slow original inspection must survive the ordinary two-second IPC deadline.
use crate::audio_probe_tests::slow_response;
use sungeki_protocol::SessionRequest;

async fn inspected_request(kind: &str) {
    let args = if kind.starts_with("start_") {
        serde_json::json!({
            "session_id":"session", "job_id":"job", "expected_revision":0,
            "clip_id":"speech", "profile_id":"local"
        })
    } else {
        serde_json::json!({"profile_id":"local"})
    };
    let request: SessionRequest = serde_json::from_value(serde_json::json!({
        "type":"speech_job", "request":{"type":kind, "args":args}
    }))
    .unwrap();
    slow_response(request, kind).await;
}

#[tokio::test]
async fn qwen_profile_waits_for_original_inspection() {
    inspected_request("get_qwen_profile").await;
}

#[tokio::test]
async fn qwen_start_waits_for_original_inspection() {
    inspected_request("start_qwen").await;
}

#[tokio::test]
async fn sbv2_profile_waits_for_original_inspection() {
    inspected_request("get_sbv2_profile").await;
}

#[tokio::test]
async fn sbv2_start_waits_for_original_inspection() {
    inspected_request("start_sbv2").await;
}

#[tokio::test]
async fn profile_lists_wait_for_large_configuration_reads() {
    use sungeki_protocol::SpeechJobRequest;
    for (request, label) in [
        (SpeechJobRequest::ListQwenProfiles, "list-qwen"),
        (SpeechJobRequest::ListSbv2Profiles, "list-sbv2"),
        (SpeechJobRequest::ListIrodoriProfiles, "list-irodori"),
    ] {
        slow_response(SessionRequest::SpeechJob { request }, label).await;
    }
}

#[tokio::test]
async fn irodori_profile_waits_for_original_inspection() {
    inspected_request("get_irodori_profile").await;
}
