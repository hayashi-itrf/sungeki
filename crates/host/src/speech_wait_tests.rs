use super::*;
use crate::{
    speech_journal::create,
    speech_journal_tests::{Fixture, id, seed},
};
use std::thread;
use sungeki_project::JobPhase;
use sungeki_protocol::{SessionRequest, SpeechApplication};

fn fixture() -> (Fixture, Arc<Mutex<Sessions>>, String) {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let row = create(&mut sessions, id(), sid, &input).unwrap();
    (fixture, Arc::new(Mutex::new(sessions)), row.id)
}

#[test]
fn speech_wait_timeout_preserves_job_and_rejects_invalid_limits() {
    let (_fixture, shared, id) = fixture();
    let before = shared.lock().unwrap().store.load_job(&id).unwrap();
    let now = Instant::now();
    let SessionResponse::SpeechJob { job } = get(&shared, &id, 40).unwrap() else {
        panic!()
    };
    assert!(now.elapsed() >= Duration::from_millis(40));
    assert_eq!(job.job_id, id);
    assert_eq!(job.phase, JobPhase::Queued);
    assert_eq!(shared.lock().unwrap().store.load_job(&id).unwrap(), before);
    assert_eq!(
        get(&shared, &id, 30_001).unwrap_err().code,
        ErrorCode::ValidationFailed
    );
    assert_eq!(
        get(&shared, "missing", 30_000).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn speech_wait_cancel_wakes_all_readers_without_blocking_sessions() {
    let (_fixture, shared, id) = fixture();
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let (shared, id) = (shared.clone(), id.clone());
            thread::spawn(move || get(&shared, &id, 5_000).unwrap())
        })
        .collect();
    thread::sleep(Duration::from_millis(50));
    Sessions::execute(
        &shared,
        SessionRequest::Create {
            title: "concurrent".into(),
        },
    )
    .unwrap();
    let now = Instant::now();
    crate::speech_runtime::cancel(&shared, &id).unwrap();
    for reader in readers {
        let SessionResponse::SpeechJob { job } = reader.join().unwrap() else {
            panic!()
        };
        assert_eq!(job.phase, JobPhase::Cancelled);
        assert_eq!(job.job_id, id);
    }
    assert!(now.elapsed() < Duration::from_secs(2));
    let now = Instant::now();
    get(&shared, &id, 30_000).unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn speech_wait_success_wakes_after_atomic_project_application() {
    let (_fixture, shared, id) = fixture();
    {
        let mut sessions = shared.lock().unwrap();
        crate::speech_dispatch::Reservation::acquire(&mut sessions, &id)
            .unwrap()
            .begin(&mut sessions)
            .unwrap()
            .confirm_finished(&mut sessions)
            .unwrap();
    }
    let s = shared.clone();
    let job_id = id.clone();
    let reader = thread::spawn(move || get(&s, &job_id, 5_000).unwrap());
    thread::sleep(Duration::from_millis(50));
    let now = Instant::now();
    crate::speech_output::complete(
        &shared,
        &id,
        crate::speech_output_tests::response(),
        &std::sync::atomic::AtomicBool::new(false),
    )
    .await
    .unwrap();
    let SessionResponse::SpeechJob { job } = reader.join().unwrap() else {
        panic!()
    };
    assert!(now.elapsed() < Duration::from_secs(2));
    assert_eq!(job.phase, JobPhase::Succeeded);
    assert!(matches!(job.application, SpeechApplication::Applied { .. }));
    assert!(!job.stale);
    assert!(job.artifact.is_some());
}

#[test]
fn speech_start_rejects_wait_before_target_or_profile_lookup() {
    let fixture = Fixture::new();
    let shared = Arc::new(Mutex::new(fixture.open()));
    for kind in ["start", "start_sbv2", "start_qwen", "start_irodori"] {
        let mut args = serde_json::json!({"session_id":"missing", "job_id":id(),"clip_id":"missing","expected_revision":0,"wait_ms":30001});
        if kind == "start" {
            args["base_url"] = "invalid".into();
            args["engine_version"] = "1".into();
        } else {
            args["profile_id"] = "missing".into();
        }
        let request = serde_json::from_value(serde_json::json!({"type":kind,"args":args})).unwrap();
        let error = crate::speech_api::execute(&shared, request).unwrap_err();
        assert_eq!(error.code, ErrorCode::ValidationFailed);
        assert_eq!(error.message, "待機は0〜30000msです");
    }
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .speech_runtime
            .active
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[test]
fn speech_start_wait_is_excluded_from_in_progress_preparation_identity() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "preparation replay".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let shared = Arc::new(Mutex::new(sessions));
    for kind in ["start_sbv2", "start_qwen", "start_irodori"] {
        let job_id = id();
        let mut value = serde_json::json!({"type":kind,"args":{"session_id":snapshot.session_id,"job_id":job_id,"clip_id":"unused","expected_revision":0,"profile_id":"not-read"}});
        let request: sungeki_protocol::SpeechJobRequest =
            serde_json::from_value(value.clone()).unwrap();
        let key = match request {
            sungeki_protocol::SpeechJobRequest::StartSbv2 { args } => {
                serde_json::to_string(&args).unwrap()
            }
            sungeki_protocol::SpeechJobRequest::StartQwen { args } => {
                serde_json::to_string(&args).unwrap()
            }
            sungeki_protocol::SpeechJobRequest::StartIrodori { args } => {
                serde_json::to_string(&args).unwrap()
            }
            _ => unreachable!(),
        };
        let preparations = shared.lock().unwrap().speech_runtime.preparations.clone();
        let _held = preparations
            .claim(&job_id, &snapshot.session_id, &key)
            .unwrap();
        for wait in [0, 30_000] {
            value["args"]["wait_ms"] = wait.into();
            let request = serde_json::from_value(value.clone()).unwrap();
            let error = crate::speech_api::execute(&shared, request).unwrap_err();
            assert_eq!(error.code, ErrorCode::HostUnavailable);
            assert!(error.retryable);
            assert_eq!(error.details["job_created"], "unknown");
            assert!(
                shared
                    .lock()
                    .unwrap()
                    .store
                    .load_job(&job_id)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
