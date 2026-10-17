use super::{absent, job::seed_qwen, request};
use crate::{
    sessions::Sessions,
    speech_journal_tests::{Fixture, id},
};
use serde_json::json;
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_protocol::*;

async fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: StartQwenSpeechArgs,
) -> Result<SessionResponse, ApiError> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || {
        Sessions::execute(
            &shared,
            SessionRequest::SpeechJob {
                request: SpeechJobRequest::StartQwen { args },
            },
        )
    })
    .await
    .unwrap()
}
fn args(sid: &str, job: &str, clip: &str, profile: &str) -> StartQwenSpeechArgs {
    StartQwenSpeechArgs {
        wait_ms: Some(10_000),
        session_id: sid.into(),
        job_id: job.into(),
        clip_id: clip.into(),
        profile_id: profile.into(),
        expected_revision: 2,
    }
}
#[tokio::test]
async fn public_qwen_start_and_replay_use_saved_profile_without_relaunch() {
    let (_engine, request, pid) = request("speech");
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_qwen(&mut sessions, &request);
    let config_path = sessions
        .review_database_path
        .parent()
        .unwrap()
        .join("qwen.json");
    let profile = json!({"id":"local-qwen","name":"Qwen", "binding":request.binding,
        "port":request.port,"identity":request.identity,"default_settings":request.settings,
        "catalog":[{"status":"healthy","backend":{"name":"official","model_id":request.identity.model_id,"ready":true},"version":"test"},
            {"data":[{"id":"qwen3-tts"}]},{"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]}]});
    std::fs::write(
        &config_path,
        serde_json::to_vec(&json!({"format_version":1,"profiles":[profile]})).unwrap(),
    )
    .unwrap();
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let response = crate::qwen_originals::profiles::list(&shared).unwrap();
    let SessionResponse::QwenProfiles { profiles } = response else {
        panic!()
    };
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, "local-qwen");
    let SessionResponse::QwenProfile { details } =
        crate::qwen_originals::profiles::get(&shared, "local-qwen").unwrap()
    else {
        panic!()
    };
    assert_eq!(details.speakers[0].id, "Vivian");
    assert_eq!(details.languages, ["Japanese"]);
    assert_eq!(details.identity, request.identity);
    assert!(!pid.exists(), "reading profiles must not start the engine");
    let job = id();
    let response = execute(&shared, args(&sid, &job, input.clip_id(), "local-qwen"))
        .await
        .unwrap();
    assert!(
        matches!(response, SessionResponse::SpeechJob { job } if job.phase == sungeki_project::JobPhase::Succeeded)
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    absent(&pid);
    let stored = shared
        .lock()
        .unwrap()
        .store
        .load_job(&job)
        .unwrap()
        .unwrap();
    let (_, state) = crate::speech_journal::decode_input(&stored).unwrap();
    assert_eq!(
        state.phase,
        sungeki_project::JobPhase::Succeeded,
        "{:?}",
        state.error
    );
    std::fs::write(request.binding.model_root.join("config.json"), "changed").unwrap();
    assert!(crate::qwen_originals::profiles::list(&shared).is_ok());
    assert_eq!(
        crate::qwen_originals::profiles::get(&shared, "local-qwen")
            .unwrap_err()
            .code,
        ErrorCode::ExternalModification
    );
    // Changed weights now fail in a durable job, before a second process starts.
    let previous_pid = std::fs::read(&pid).unwrap();
    let failed_id = id();
    let failed_revision = shared
        .lock()
        .unwrap()
        .load(&sid)
        .unwrap()
        .1
        .history()
        .revision();
    let failed_args = || {
        let mut value = args(&sid, &failed_id, input.clip_id(), "local-qwen");
        value.expected_revision = failed_revision;
        value
    };
    execute(&shared, failed_args()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let failed = shared
        .lock()
        .unwrap()
        .store
        .load_job(&failed_id)
        .unwrap()
        .unwrap();
    let (_, failed_state) = crate::speech_journal::decode_input(&failed).unwrap();
    assert_eq!(failed_state.phase, sungeki_project::JobPhase::Failed);
    assert_eq!(
        failed_state.error.unwrap().code,
        ErrorCode::ExternalModification
    );
    assert!(failed_state.artifact.is_none());
    assert_eq!(std::fs::read(&pid).unwrap(), previous_pid);
    absent(&pid);
    std::fs::remove_file(&config_path).unwrap();
    execute(&shared, failed_args()).await.unwrap();
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&failed_id)
            .unwrap()
            .unwrap(),
        failed
    );
    execute(&shared, args(&sid, &job, input.clip_id(), "local-qwen"))
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
        stored
    );
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    assert_eq!(
        execute(&shared, args(&sid, &job, input.clip_id(), "other"))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let mut changed = args(&sid, &job, input.clip_id(), "local-qwen");
    changed.expected_revision = 3;
    assert_eq!(
        execute(&shared, changed).await.unwrap_err().code,
        ErrorCode::Conflict
    );
}
