use super::{job::jobs, startup::request};
use crate::{
    sessions::Sessions,
    speech_journal::{Input, decode_input},
    speech_journal_tests::id,
    speech_runtime::{self, Runtime},
    speech_state::Execution,
};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_project::JobPhase;
use sungeki_protocol::ErrorCode;

async fn idle(runtime: &Runtime) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
fn input(sessions: &Sessions, job: &str) -> (String, sungeki_speech::sbv2::Sbv2JobInput) {
    let stored = sessions.store.load_job(job).unwrap().unwrap();
    let (Input::Sbv2(input), _) = decode_input(&stored).unwrap() else {
        panic!()
    };
    (stored.session_id, input)
}
#[tokio::test]
async fn scheduler_runs_sbv2_once_and_replay_does_not_use_changed_launch_configuration() {
    let (fixture, request, pid) = request("speech");
    let mut sessions = fixture.open();
    let (seed, _) = jobs(&mut sessions, &request);
    let (sid, input) = input(&sessions, &seed);
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let first = speech_runtime::start_sbv2(
        &shared,
        sid.clone(),
        id(),
        &input,
        request.binding.clone(),
        true,
    )
    .unwrap();
    let replay = speech_runtime::start_sbv2(
        &shared,
        sid.clone(),
        first.id.clone(),
        &input,
        request.binding.clone(),
        true,
    )
    .unwrap();
    assert_eq!(first, replay);
    idle(&runtime).await;
    let completed = shared
        .lock()
        .unwrap()
        .store
        .load_job(&first.id)
        .unwrap()
        .unwrap();
    let (_, state) = decode_input(&completed).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert_eq!(state.execution, Execution::Finished);
    let engine_pid = std::fs::read_to_string(&pid).unwrap();
    let original_bytes = std::fs::read(state.artifact.unwrap().path).unwrap();
    let mut broken = request.binding;
    broken.endpoint = "http://unconfigured.invalid".into();
    assert_eq!(
        speech_runtime::start_sbv2(&shared, sid, first.id, &input, broken, false).unwrap(),
        completed
    );
    idle(&runtime).await;
    assert_eq!(std::fs::read_to_string(pid).unwrap(), engine_pid);
    assert_eq!(
        original_bytes,
        std::fs::read(fixture.0.join("response.wav")).unwrap()
    );
}
#[tokio::test]
async fn queued_cancel_never_starts_engine_and_running_cancel_uses_owned_shutdown() {
    let (fixture, request, pid) = request("speech_hang");
    let mut sessions = fixture.open();
    let (seed, _) = jobs(&mut sessions, &request);
    let (sid, input) = input(&sessions, &seed);
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let first = speech_runtime::start_sbv2(
        &shared,
        sid.clone(),
        id(),
        &input,
        request.binding.clone(),
        true,
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !fixture.0.join("voice.request").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let engine_pid = std::fs::read_to_string(&pid).unwrap();
    let second =
        speech_runtime::start_sbv2(&shared, sid, id(), &input, request.binding, true).unwrap();
    let cancelled = speech_runtime::cancel(&shared, &second.id).unwrap();
    assert_eq!(
        decode_input(&cancelled).unwrap().1.execution,
        Execution::NotDispatched
    );
    speech_runtime::cancel(&shared, &first.id).unwrap();
    idle(&runtime).await;
    let sessions = shared.lock().unwrap();
    for job in [&first.id, &second.id] {
        let (_, state) = decode_input(&sessions.store.load_job(job).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, JobPhase::Cancelled);
        assert!(state.artifact.is_none());
        assert_eq!(
            state.execution,
            if job == &first.id {
                Execution::Finished
            } else {
                Execution::NotDispatched
            }
        );
    }
    assert_eq!(std::fs::read_to_string(pid).unwrap(), engine_pid);
    assert_eq!(unsafe { libc::kill(engine_pid.parse().unwrap(), 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
}
#[tokio::test]
async fn wrong_binding_is_rejected_before_creating_or_scheduling_a_job() {
    let (fixture, mut request, pid) = request("speech");
    let mut sessions = fixture.open();
    let (seed, _) = jobs(&mut sessions, &request);
    let (sid, input) = input(&sessions, &seed);
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    request.binding.endpoint = "http://127.0.0.1:1024".into();
    // Fixture ephemeral ports cannot be 1024.
    let job = id();
    assert_eq!(
        speech_runtime::start_sbv2(&shared, sid, job.clone(), &input, request.binding, true)
            .unwrap_err()
            .code,
        ErrorCode::ValidationFailed
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
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    assert!(!pid.exists());
}
