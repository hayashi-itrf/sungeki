use super::*;
use crate::{
    speech_exchange_tests::support::*,
    speech_journal_tests::{Fixture, id, seed, seed_at},
};
use std::time::Duration;
async fn idle(runtime: &Runtime) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn bounded_queue_replay_and_cancel_do_not_wait_for_execution_slot() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let runtime = sessions.speech_runtime.clone();
    let held = runtime.slots.clone().acquire_owned().await.unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let mut jobs = vec![];
    for _ in 0..8 {
        jobs.push(start(&shared, sid.clone(), id(), &input).unwrap());
    }
    let ninth = id();
    assert_eq!(
        start(&shared, sid.clone(), ninth.clone(), &input)
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    assert!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&ninth)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        start(&shared, sid.clone(), jobs[0].id.clone(), &input).unwrap(),
        jobs[0]
    );
    assert_eq!(runtime.active.load(Ordering::SeqCst), 8);
    for job in jobs {
        let cancelled = cancel(&shared, &job.id).unwrap();
        assert_eq!(decode(&cancelled).unwrap().1.phase, JobPhase::Cancelled);
        assert_eq!(cancel(&shared, &job.id).unwrap(), cancelled);
        assert_eq!(
            start(&shared, sid.clone(), job.id, &input).unwrap(),
            cancelled
        );
    }
    idle(&runtime).await; // permit is STILL held; no network or semaphore wait
    assert!(runtime.controls.lock().unwrap().is_empty());
    assert!(
        shared
            .lock()
            .unwrap()
            .speech_resources
            .quarantined()
            .unwrap()
            .is_empty()
    );
    drop(held);
}
#[tokio::test]
async fn scheduler_runs_once_and_completed_replay_retains_same_artifact() {
    let mut replies = metadata(1);
    let mut audio = Reply::new("/synthesis", "");
    audio.body = crate::speech_output_tests::response().unverified_wav;
    replies.extend([analysis(), audio]);
    let (base, server) = server(replies).await;
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_at(&mut sessions, &base, ENGINE);
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let first = start(&shared, sid.clone(), id(), &input).unwrap();
    assert_eq!(
        start(&shared, sid.clone(), first.id.clone(), &input).unwrap(),
        first
    );
    assert_eq!(server.await.unwrap().len(), 12);
    idle(&runtime).await;
    let completed = start(&shared, sid.clone(), first.id.clone(), &input).unwrap();
    assert_eq!(decode(&completed).unwrap().1.phase, JobPhase::Succeeded);
    assert_eq!(cancel(&shared, &first.id).unwrap(), completed);
    let path = decode(&completed).unwrap().1.artifact.unwrap().path;
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(start(&shared, sid, first.id, &input).unwrap(), completed);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn quarantined_engine_failure_does_not_leave_a_job_queued_forever() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    sessions.speech_resources =
        sungeki_jobs::Resources::recover(vec![], vec![(id(), vec![sungeki_jobs::Resource::Gpu])])
            .unwrap();
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let job = start(&shared, sid, id(), &input).unwrap();
    idle(&runtime).await;
    let sessions = shared.lock().unwrap();
    let (_, state) = decode(&sessions.store.load_job(&job.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Failed);
    assert_eq!(
        state.execution,
        crate::speech_state::Execution::NotDispatched
    );
    assert_eq!(state.error.unwrap().code, ErrorCode::Conflict);
    assert_eq!(sessions.speech_resources.quarantined().unwrap().len(), 1);
}
