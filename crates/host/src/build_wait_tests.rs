use super::*;
use crate::build_record::{execute, request_cancel, settle_cancel};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use sungeki_protocol::{BuildVideoRequest, ErrorCode, SessionResponse};

#[test]
fn finite_wait_keeps_job_and_session_unchanged() {
    let (root, sessions, row) = super::reservation_tests::setup(false);
    let shared = Arc::new(Mutex::new(sessions));
    let before = shared.lock().unwrap().store.load("session").unwrap();
    let started = Instant::now();
    let SessionResponse::BuildVideoJob { job } = execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: row.id.clone(),
            wait_ms: 40,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert!(started.elapsed() >= Duration::from_millis(40));
    assert_eq!(job.job_id, row.id);
    assert_eq!(job.phase, JobPhase::Queued);
    assert_eq!(
        shared.lock().unwrap().store.load_job(&row.id).unwrap(),
        Some(row.clone())
    );
    assert_eq!(
        shared.lock().unwrap().store.load("session").unwrap(),
        before
    );
    assert_eq!(
        execute(
            &shared,
            BuildVideoRequest::Get {
                job_id: row.id,
                wait_ms: 30_001
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::ValidationFailed
    );
    assert_eq!(
        execute(
            &shared,
            BuildVideoRequest::Get {
                job_id: "missing".into(),
                wait_ms: 30_000
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn all_waiters_wake_on_cancel_without_blocking_other_edits() {
    let (root, sessions, row) = super::reservation_tests::setup(false);
    let shared = Arc::new(Mutex::new(sessions));
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let shared = shared.clone();
            let id = row.id.clone();
            thread::spawn(move || {
                execute(
                    &shared,
                    BuildVideoRequest::Get {
                        job_id: id,
                        wait_ms: 5_000,
                    },
                )
                .unwrap()
            })
        })
        .collect();
    thread::sleep(Duration::from_millis(50));
    Sessions::execute(
        &shared,
        sungeki_protocol::SessionRequest::Create {
            title: "待機中も編集".into(),
        },
    )
    .unwrap();
    let started = Instant::now();
    {
        let mut sessions = shared.lock().unwrap();
        let cancelled = request_cancel(&mut sessions, &row).unwrap();
        assert!(settle_cancel(&mut sessions, &cancelled).unwrap());
    }
    for reader in readers {
        let SessionResponse::BuildVideoJob { job } = reader.join().unwrap() else {
            panic!()
        };
        assert_eq!(job.phase, JobPhase::Cancelled);
        assert_eq!(job.job_id, row.id);
    }
    assert!(started.elapsed() < Duration::from_secs(2));
    let started = Instant::now();
    execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: row.id,
            wait_ms: 30_000,
        },
    )
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
