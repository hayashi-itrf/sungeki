pub(crate) mod support;
use crate::{
    speech_exchange::run,
    speech_journal::{create, decode},
    speech_journal_tests::{Fixture, id, seed_at},
    speech_state::Execution,
};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_project::JobPhase;
use sungeki_protocol::ErrorCode;
use support::*;

#[tokio::test]
async fn exchange_uses_frozen_identity_and_finishes_execution_without_claiming_valid_audio() {
    let mut replies = metadata(1);
    replies.extend([analysis(), Reply::new("/synthesis", "not a wav")]);
    let (base, server) = server(replies).await;
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_at(&mut sessions, &base, ENGINE);
    let job = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let response = run(
        &shared,
        &job.id,
        &AtomicBool::new(false),
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert!(response.verify().is_err()); // HTTP success is NOT an audio artifact
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 12);
    assert!(requests[10].contains("speaker=1&core_version=1"));
    assert!(requests[11].contains("speaker=1&core_version=1"));
    let sessions = shared.lock().unwrap();
    let (_, state) = decode(&sessions.store.load_job(&job.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Running);
    assert_eq!(state.execution, Execution::Finished);
    assert!(state.artifact.is_none());
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    assert_eq!(
        sessions.load(&sid).unwrap().1.history().project(),
        input.project()
    );
}

#[tokio::test]
async fn changed_identity_or_missing_style_prevents_synthesis_and_records_failure() {
    for changed_version in [true, false] {
        let mut replies = metadata(99);
        if changed_version {
            replies.truncate(3);
            replies[1].body = "\"different\"".into();
        }
        let expected = replies.len();
        let (base, server) = server(replies).await;
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = seed_at(&mut sessions, &base, ENGINE);
        let job = create(&mut sessions, id(), sid, &input).unwrap();
        let shared = Arc::new(Mutex::new(sessions));
        let error = run(
            &shared,
            &job.id,
            &AtomicBool::new(false),
            Duration::from_secs(1),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::EngineUnavailable);
        assert_eq!(server.await.unwrap().len(), expected);
        let sessions = shared.lock().unwrap();
        let (_, state) = decode(&sessions.store.load_job(&job.id).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, JobPhase::Failed);
        assert_eq!(state.execution, Execution::NotDispatched);
        assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    }
}

#[tokio::test]
async fn cancellation_drains_response_while_timeout_retains_quarantine_after_recovery() {
    for timeout in [false, true] {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut replies = metadata(1);
        let mut reply = analysis();
        reply.cancel = Some(cancelled.clone());
        reply.delay = Duration::from_millis(if timeout { 200 } else { 10 });
        replies.push(reply);
        let (base, server) = server(replies).await;
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = seed_at(&mut sessions, &base, ENGINE);
        let job = create(&mut sessions, id(), sid, &input).unwrap();
        let shared = Arc::new(Mutex::new(sessions));
        assert!(
            run(&shared, &job.id, &cancelled, Duration::from_millis(100))
                .await
                .is_err()
        );
        assert_eq!(server.await.unwrap().len(), 11); // never synthesis after cancel
        {
            let sessions = shared.lock().unwrap();
            let (_, state) = decode(&sessions.store.load_job(&job.id).unwrap().unwrap()).unwrap();
            assert_eq!(
                state.phase,
                if timeout {
                    JobPhase::Interrupted
                } else {
                    JobPhase::Cancelled
                }
            );
            assert_eq!(
                state.execution,
                if timeout {
                    Execution::PossiblyRunning
                } else {
                    Execution::Finished
                }
            );
            assert_eq!(
                sessions.speech_resources.quarantined().unwrap().len(),
                usize::from(timeout)
            );
        }
        drop(shared);
        assert_eq!(
            fixture.open().speech_resources.quarantined().unwrap().len(),
            usize::from(timeout)
        );
    }
}

#[tokio::test]
async fn persisted_cancel_wins_without_flag_before_dispatch_and_after_response() {
    for during_synthesis in [false, true] {
        let fixture = Fixture::new();
        let shared = Arc::new(Mutex::new(fixture.open()));
        let job_id = id();
        let mut replies = metadata(1);
        if during_synthesis {
            replies.extend([analysis(), Reply::new("/synthesis", "discard me")]);
        }
        let expected = replies.len();
        let hook_shared = shared.clone();
        let hook_id = job_id.clone();
        replies.last_mut().unwrap().on_request = Some(Box::new(move || {
            // Proves the host session mutex is not held across metadata/synthesis I/O.
            let mut sessions = hook_shared.try_lock().unwrap();
            let record = sessions.store.load_job(&hook_id).unwrap().unwrap();
            let (_, mut state) = decode(&record).unwrap();
            state.phase = JobPhase::Cancelling;
            crate::speech_journal::save(&mut sessions, &record, &state).unwrap();
        }));
        let (base, server) = server(replies).await;
        {
            let mut sessions = shared.lock().unwrap();
            let (sid, input) = seed_at(&mut sessions, &base, ENGINE);
            create(&mut sessions, job_id.clone(), sid, &input).unwrap();
        }
        let result = run(
            &shared,
            &job_id,
            &AtomicBool::new(false),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(result.err().unwrap().code, ErrorCode::Cancelled);
        assert_eq!(server.await.unwrap().len(), expected);
        let sessions = shared.lock().unwrap();
        let (_, state) = decode(&sessions.store.load_job(&job_id).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, JobPhase::Cancelled);
        assert_eq!(
            state.execution,
            if during_synthesis {
                Execution::Finished
            } else {
                Execution::NotDispatched
            }
        );
        assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    }
}

#[tokio::test]
async fn http_to_persisted_wav_completes_internal_worker() {
    let mut replies = metadata(1);
    let mut audio = Reply::new("/synthesis", "");
    audio.body = crate::speech_output_tests::response().unverified_wav;
    replies.extend([analysis(), audio]);
    let (base, server) = server(replies).await;
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_at(&mut sessions, &base, ENGINE);
    let job = create(&mut sessions, id(), sid, &input).unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    crate::speech_output::run(
        &shared,
        &job.id,
        &AtomicBool::new(false),
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert_eq!(server.await.unwrap().len(), 12);
    drop(shared);
    let sessions = fixture.open();
    let (_, state) = decode(&sessions.store.load_job(&job.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    let artifact = state.artifact.unwrap();
    assert_eq!(
        std::fs::read(&artifact.path).unwrap(),
        crate::speech_output_tests::response().unverified_wav
    );
    assert_eq!(artifact.duration_us, 1000);
}
