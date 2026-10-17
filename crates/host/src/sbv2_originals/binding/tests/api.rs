use super::{job::jobs, startup::request};
use crate::{sessions::Sessions, speech_journal::decode_input, speech_journal_tests::id};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_protocol::*;

fn args(sid: &str, job: &str, clip: &str, profile: &str) -> StartSbv2SpeechArgs {
    StartSbv2SpeechArgs {
        wait_ms: Some(10_000),
        session_id: sid.into(),
        job_id: job.into(),
        expected_revision: 2,
        clip_id: clip.into(),
        profile_id: profile.into(),
    }
}
async fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: StartSbv2SpeechArgs,
) -> Result<SessionResponse, ApiError> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || {
        Sessions::execute(
            &shared,
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::StartSbv2 { args },
            },
        )
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn changed_weights_are_rejected_inside_durable_job_before_engine_launch() {
    let (fixture, request, pid) = request("speech");
    let config = serde_json::json!({"format_version":1,"profiles":[{"id":"local-jp","name":"日本語",
        "binding":request.binding,"cpu_only":true,"catalog":request.catalog.models()}]});
    std::fs::write(
        fixture.0.join("sbv2.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let mut sessions = fixture.open();
    let (seed, _) = jobs(&mut sessions, &request);
    let record = sessions.store.load_job(&seed).unwrap().unwrap();
    let (input, _) = decode_input(&record).unwrap();
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    std::fs::write(&request.binding.originals.model, b"changed weights").unwrap();
    let job = id();
    let start = args(&record.session_id, &job, input.clip_id(), "local-jp");
    assert!(matches!(
        execute(
            &shared,
            args(&record.session_id, &job, input.clip_id(), "local-jp")
        )
        .await
        .unwrap(),
        SessionResponse::SpeechJob { job } if job.phase.is_terminal()
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let stored = shared
        .lock()
        .unwrap()
        .store
        .load_job(&job)
        .unwrap()
        .unwrap();
    let (_, state) = decode_input(&stored).unwrap();
    assert_eq!(state.phase, sungeki_project::JobPhase::Failed);
    assert_eq!(state.error.unwrap().code, ErrorCode::ExternalModification);
    assert!(state.artifact.is_none());
    assert!(!pid.exists());
    // Replay is the same terminal job, even after originals are removed.
    std::fs::remove_file(&request.binding.originals.model).unwrap();
    execute(&shared, start).await.unwrap();
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&job)
            .unwrap()
            .unwrap(),
        stored
    );
}
#[tokio::test]
async fn public_start_uses_named_profile_and_replay_never_reloads_changed_settings() {
    let (fixture, request, pid) = request("speech");
    let config = serde_json::json!({"format_version":1,"profiles":[{"id":"local-jp","name":"日本語",
        "binding":request.binding,"cpu_only":true,"catalog":request.catalog.models()}]});
    let config_path = fixture.0.join("sbv2.json");
    std::fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut sessions = fixture.open();
    let (seed, _) = jobs(&mut sessions, &request);
    let record = sessions.store.load_job(&seed).unwrap().unwrap();
    let (input, _) = decode_input(&record).unwrap();
    let sid = record.session_id;
    let clip = input.clip_id().to_string();
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let job = id();
    assert!(matches!(
        execute(&shared, args(&sid, &job, &clip, "local-jp"))
            .await
            .unwrap(),
        SessionResponse::SpeechJob { job } if job.phase.is_terminal()
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let completed = shared
        .lock()
        .unwrap()
        .store
        .load_job(&job)
        .unwrap()
        .unwrap();
    let (input, state) = decode_input(&completed).unwrap();
    assert_eq!(state.phase, sungeki_project::JobPhase::Succeeded);
    let crate::speech_journal::Input::Sbv2(input) = input else {
        panic!()
    };
    assert_eq!(input.profile_id(), Some("local-jp"));
    let original_pid = std::fs::read_to_string(&pid).unwrap();
    std::fs::remove_file(config_path).unwrap();
    execute(&shared, args(&sid, &job, &clip, "local-jp"))
        .await
        .unwrap();
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&job)
            .unwrap()
            .unwrap(),
        completed
    );
    assert_eq!(std::fs::read_to_string(pid).unwrap(), original_pid);
    assert_eq!(
        execute(&shared, args(&sid, &job, &clip, "other"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let mut changed = args(&sid, &job, &clip, "local-jp");
    changed.expected_revision += 1;
    assert_eq!(
        execute(&shared, changed).await.unwrap_err().code,
        ErrorCode::Conflict
    );
}
#[tokio::test]
async fn missing_profile_or_changed_revision_creates_no_job_and_starts_no_engine() {
    let (fixture, request, pid) = request("speech");
    let mut sessions = fixture.open();
    let (seed, _) = jobs(&mut sessions, &request);
    let record = sessions.store.load_job(&seed).unwrap().unwrap();
    let (input, _) = decode_input(&record).unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let job = id();
    assert_eq!(
        execute(
            &shared,
            args(&record.session_id, &job, input.clip_id(), "local-jp")
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnavailable
    );
    let mut stale = args(&record.session_id, &job, input.clip_id(), "local-jp");
    stale.expected_revision = 1;
    assert_eq!(
        execute(&shared, stale).await.unwrap_err().code,
        ErrorCode::Conflict
    );
    assert!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&job)
            .unwrap()
            .is_none()
    );
    assert!(!pid.exists());
}
