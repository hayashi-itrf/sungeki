use super::{Request, fixture, gone};
use crate::irodori_originals::exchange;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_speech::irodori::{
    Catalog, ModelCapabilities, PreparedIrodoriInput, ReferenceKind, RegisteredVoice,
};
fn input(request: &Request) -> PreparedIrodoriInput {
    let catalog = Catalog::new(
        ModelCapabilities {
            checkpoint: "model".into(),
            speaker_conditioning: true,
            caption_conditioning: false,
            sample_rate: 48000,
            channels: 1,
        },
        vec![RegisteredVoice {
            id: "none".into(),
            name: "参照なし".into(),
            reference: ReferenceKind::None,
            caption: None,
        }],
    )
    .unwrap();
    let selected = catalog
        .select(&request.settings, &request.identity)
        .unwrap();
    PreparedIrodoriInput::from_selected("合成検査", None, &[], &selected).unwrap()
}
#[tokio::test]
async fn verified_audio_is_returned_only_after_owned_child_stops() {
    let (_f, request, pid) = fixture("speech");
    let audio = input(&request);
    let result = exchange::run(
        request,
        audio,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap_or_else(|f| panic!("{:?}", f.error));
    assert_eq!(result.info().frames, 480);
    assert_eq!(result.info().duration_us, 10000);
    assert!(pid.exists());
    gone(&pid);
}
#[tokio::test]
async fn failed_audio_and_changed_originals_never_escape_and_child_is_reaped() {
    for mode in [
        "speech_bad",
        "speech_wrong_rate",
        "speech_changed",
        "speech_hang",
        "cancel",
    ] {
        let (_f, request, pid) = fixture(if mode == "cancel" {
            "speech_hang"
        } else {
            mode
        });
        let audio = input(&request);
        let marker = request.binding.work_root.join("speech.started");
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let path = marker.clone();
        let trigger = (mode == "cancel").then(|| {
            std::thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(3);
                while !path.exists() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                flag.store(true, Ordering::Release);
            })
        });
        let failure = exchange::run(request, audio, cancel, Duration::from_secs(2))
            .await
            .err()
            .unwrap();
        if let Some(trigger) = trigger {
            trigger.join().unwrap();
        }
        assert!(failure.cleanup_confirmed, "{mode}: {:?}", failure.error);
        assert!(failure.unsettled.is_none());
        use sungeki_protocol::ErrorCode;
        let code = match mode {
            "speech_bad" | "speech_wrong_rate" => ErrorCode::ValidationFailed,
            "speech_changed" => ErrorCode::ExternalModification,
            "cancel" => ErrorCode::Cancelled,
            _ => ErrorCode::EngineUnavailable,
        };
        assert_eq!(failure.error.code, code, "{mode}: {:?}", failure.error);
        assert!(marker.exists(), "must reach synthesis: {mode}");
        gone(&pid);
    }
}
#[tokio::test]
async fn mismatched_input_rejected_before_launch() {
    let (_f, mut request, pid) = fixture("speech");
    let audio = input(&request);
    request.settings.seed += 1;
    let failure = exchange::run(
        request,
        audio,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(2),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        failure.error.code,
        sungeki_protocol::ErrorCode::ValidationFailed
    );
    assert!(!pid.exists());
}
