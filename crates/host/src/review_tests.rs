use crate::{review_jobs, sessions::Sessions};
use std::sync::{Arc, Mutex, atomic::Ordering};
use sungeki_project::JobPhase;
use sungeki_protocol::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bounded_queue_cancellation_and_fixed_input() {
    let directory = std::env::temp_dir().join(format!("vvm-review-queue-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let shared = Arc::new(Mutex::new(Sessions::open(&directory.join("db")).unwrap()));
    let SessionResponse::Snapshot { snapshot } = shared
        .lock()
        .unwrap()
        .request(SessionRequest::Create {
            title: "待機と取消".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let invalid_margin = ReviewFrameOptions {
        text_safe_margin_percent: Some(26),
        ..Default::default()
    };
    assert!(
        review_jobs::start(
            &shared,
            snapshot.session_id.clone(),
            uuid::Uuid::new_v4().to_string(),
            0,
            invalid_margin
        )
        .is_err()
    );
    let runtime = shared.lock().unwrap().review_runtime.clone();
    // Hold the actual execution slot so cancellation/queue bounds are deterministic.
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    let mut ids = Vec::new();
    for _ in 0..8 {
        let id = uuid::Uuid::new_v4().to_string();
        review_jobs::start(
            &shared,
            snapshot.session_id.clone(),
            id.clone(),
            0,
            Default::default(),
        )
        .unwrap();
        review_jobs::start(
            &shared,
            snapshot.session_id.clone(),
            id.clone(),
            0,
            Default::default(),
        )
        .unwrap();
        let changed = sungeki_protocol::ReviewFrameOptions {
            max_frames: 13,
            ..Default::default()
        };
        assert_eq!(
            review_jobs::start(&shared, snapshot.session_id.clone(), id.clone(), 0, changed)
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        let changed_margin = ReviewFrameOptions {
            text_safe_margin_percent: Some(5),
            ..Default::default()
        };
        assert_eq!(
            review_jobs::start(
                &shared,
                snapshot.session_id.clone(),
                id.clone(),
                0,
                changed_margin
            )
            .unwrap_err()
            .code,
            ErrorCode::Conflict
        );
        ids.push(id);
    }
    assert_eq!(runtime.active.load(Ordering::SeqCst), 8);
    let rejected = review_jobs::start(
        &shared,
        snapshot.session_id.clone(),
        uuid::Uuid::new_v4().to_string(),
        0,
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(rejected.code, ErrorCode::CapabilityUnavailable);
    let SessionResponse::ReviewJob { job } = review_jobs::cancel(&shared, ids[0].clone()).unwrap()
    else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Cancelling);
    assert!(job.report.is_none());
    let SessionResponse::ReviewJob { job } = review_jobs::get(&shared, ids[0].clone(), 1).unwrap()
    else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Cancelling); // finite wait doesn't claim completion
    shared
        .lock()
        .unwrap()
        .request(SessionRequest::Edit {
            session_id: snapshot.session_id.clone(),
            edit: EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: 0,
                action: EditAction::Apply {
                    commands: vec![sungeki_editor::Command::RenameProject {
                        title: "診断待機中に編集".into(),
                    }],
                },
            },
        })
        .unwrap();
    drop(slot);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    for (index, id) in ids.iter().enumerate() {
        let SessionResponse::ReviewJob { job } = review_jobs::get(&shared, id.clone(), 0).unwrap()
        else {
            panic!()
        };
        assert!(job.stale);
        assert_eq!(job.source_revision, 0);
        if index == 0 {
            assert_eq!(job.phase, JobPhase::Cancelled);
            assert!(job.report.is_none());
        } else {
            assert_eq!(job.phase, JobPhase::Succeeded);
            assert_eq!(job.report.unwrap().diagnostics.summary.not_run, 4);
        }
    }
    assert_eq!(
        std::fs::read_dir(shared.lock().unwrap().review_directory.clone().unwrap())
            .unwrap()
            .count(),
        0
    );
    assert!(runtime.cancellations.lock().unwrap().is_empty());
    drop(shared);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unavailable_work_storage_blocks_only_new_review_not_editing() {
    let directory =
        std::env::temp_dir().join(format!("vvm-review-unavailable-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let mut sessions = Sessions::open(&directory.join("db")).unwrap();
    let work = sessions.review_directory.as_ref().unwrap().clone();
    std::fs::remove_dir(&work).unwrap();
    std::fs::write(&work, b"unavailable work directory").unwrap();
    sessions.review_directory = Err(ApiError::new(ErrorCode::IoError, "SSD未接続"));
    let shared = Arc::new(Mutex::new(sessions));
    let SessionResponse::Snapshot { snapshot } = shared
        .lock()
        .unwrap()
        .request(SessionRequest::Create {
            title: "SSDなしでも編集".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let id = uuid::Uuid::new_v4().to_string();
    let rejected = review_jobs::start(
        &shared,
        snapshot.session_id.clone(),
        id.clone(),
        0,
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(rejected.code, ErrorCode::CapabilityUnavailable);
    assert_eq!(
        rejected.details.get("job_created").map(String::as_str),
        Some("false")
    );
    assert!(rejected.retryable);
    assert!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&id)
            .unwrap()
            .is_none()
    );
    assert!(
        shared
            .lock()
            .unwrap()
            .request(SessionRequest::Get {
                session_id: snapshot.session_id
            })
            .is_ok()
    );
    drop(shared);
    std::fs::remove_dir_all(directory).unwrap();
}

#[path = "review_startup_tests.rs"]
mod startup;
