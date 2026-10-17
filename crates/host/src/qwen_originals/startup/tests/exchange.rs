use super::{absent, request};
use crate::qwen_originals::exchange;
use serde_json::json;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_media::ExpectedAudio;
use sungeki_speech::qwen::{Catalog, PreparedQwenInput};
fn input(request: &super::Request) -> PreparedQwenInput {
    let catalog=Catalog::parse(json!({"status":"healthy","backend":{"name":"official","model_id":request.identity.model_id,"ready":true},"version":"test"}),json!({"data":[{"id":"qwen3-tts"}]}),json!({"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]})).unwrap();
    PreparedQwenInput::new(
        "テスト",
        None,
        &[],
        &request.settings,
        &catalog,
        ExpectedAudio {
            sample_rate: 24000,
            channels: 1,
        },
    )
    .unwrap()
    .bind_identity(&request.identity)
    .unwrap()
}
#[tokio::test]
async fn audio_is_verified_and_child_stopped_before_return() {
    let (_fixture, request, pid) = request("speech");
    let input = input(&request);
    let result = exchange::run(
        request,
        input,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap_or_else(|f| panic!("{}", f.error));
    assert_eq!(result.info().duration_us, 10000);
    absent(&pid);
}
#[tokio::test]
async fn cold_start_can_exceed_the_synthesis_request_budget() {
    let (_fixture, request, pid) = request("slow_start");
    let input = input(&request);
    let result = exchange::run(
        request,
        input,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(1),
    )
    .await
    .unwrap_or_else(|f| panic!("{}", f.error));
    assert_eq!(result.info().duration_us, 10000);
    absent(&pid);
}
#[tokio::test]
async fn bad_audio_changed_original_timeout_and_cancel_stop_child() {
    for mode in [
        "speech_bad",
        "speech_wrong_rate",
        "speech_changed",
        "speech_hang",
        "cancel",
    ] {
        let (fixture, request, pid) = request(if mode == "cancel" {
            "speech_hang"
        } else {
            mode
        });
        let input = input(&request);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let task = tokio::spawn(async move {
            exchange::run(request, input, flag, Duration::from_secs(5)).await
        });
        if mode == "cancel" {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            while !fixture.0.join("speech.started").exists() {
                assert!(tokio::time::Instant::now() < deadline);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            cancel.store(true, Ordering::Release);
        }
        let failure = match task.await.unwrap() {
            Ok(_) => panic!("accepted {mode}"),
            Err(f) => f,
        };
        assert!(failure.cleanup_confirmed, "{}", failure.error);
        assert!(failure.unsettled.is_none());
        let expected = match mode {
            "speech_bad" | "speech_wrong_rate" => sungeki_protocol::ErrorCode::ValidationFailed,
            "speech_changed" => sungeki_protocol::ErrorCode::ExternalModification,
            "cancel" => sungeki_protocol::ErrorCode::Cancelled,
            _ => sungeki_protocol::ErrorCode::EngineUnavailable,
        };
        assert_eq!(failure.error.code, expected, "{mode}: {}", failure.error);
        assert!(
            fixture.0.join("speech.started").exists(),
            "{mode}: synthesis never started: {}",
            failure.error
        );
        assert!(
            pid.exists(),
            "{mode}: child never started: {}",
            failure.error
        );
        absent(&pid);
    }
}
