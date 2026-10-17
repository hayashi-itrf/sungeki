use super::*;
use crate::build_record::{Reservation, reserve};
use std::sync::{Arc, Mutex, atomic::Ordering};
use sungeki_protocol::SessionResponse;

pub(super) async fn ready() -> (std::path::PathBuf, Arc<Mutex<Sessions>>, StoredJob) {
    let (root, shared, parent, _) = super::render_result_tests::ready().await;
    crate::build_record::accept_render(&shared, &parent).unwrap();
    let parent = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
        let Reservation::New(parent) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        parent
    };
    (root, shared, parent)
}
pub(super) async fn idle(runtime: &crate::review_jobs::Runtime) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn parent_review_runs_common_image_checks_and_replays_without_a_second_worker() {
    let (root, shared, parent) = ready().await;
    let (_, state) = decode(&parent).unwrap();
    let id = state.child_id.unwrap();
    let runtime = shared.lock().unwrap().review_runtime.clone();
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    for _ in 0..2 {
        let SessionResponse::ReviewJob { job } =
            crate::build_record::start_review(&shared, &parent).unwrap()
        else {
            panic!()
        };
        assert_eq!(job.job_id, id);
        assert_eq!(job.phase, JobPhase::Queued);
    }
    assert_eq!(runtime.active.load(Ordering::SeqCst), 1);
    assert!(
        crate::review_jobs::start(
            &shared,
            parent.session_id.clone(),
            id.clone(),
            state.expected_revision,
            Default::default(),
        )
        .is_err()
    );
    drop(slot);
    idle(&runtime).await;
    let sessions = shared.lock().unwrap();
    let child = sessions.store.load_job(&id).unwrap().unwrap();
    let (input, result) = crate::review_record::decode(&child).unwrap();
    assert_eq!(input.parent_job_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(result.phase, JobPhase::Succeeded);
    let report = result.report.unwrap();
    assert_eq!(report.source_revision, state.expected_revision);
    assert_eq!(report.content_hash, input.content_hash);
    assert!(report.diagnostics.summary.not_run > 0);
    // Byte identity is verified even though volume/listening remain uninspected.
    assert_eq!(
        report.asset_fingerprint,
        Some(
            sungeki_review::current_source_fingerprint(&input.project.assets, 512 * 1024 * 1024,)
                .unwrap()
        )
    );
    assert!(report.diagnostics.checks.iter().any(|c| {
        c.kind == sungeki_review::CheckKind::AssetIntegrity
            && c.status == sungeki_review::CheckStatus::NotRun
    }));
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        parent
    );
    assert!(runtime.cancellations.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_dir(sessions.review_directory.as_ref().unwrap())
            .unwrap()
            .count(),
        0
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn parent_cancellation_stops_queued_review_and_keeps_parent_unsettled() {
    let (root, shared, parent) = ready().await;
    let id = decode(&parent).unwrap().1.child_id.unwrap();
    let runtime = shared.lock().unwrap().review_runtime.clone();
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    crate::build_record::start_review(&shared, &parent).unwrap();
    let cancelling = {
        let mut sessions = shared.lock().unwrap();
        let row = crate::build_record::request_cancel(&mut sessions, &parent).unwrap();
        assert!(runtime.cancellations.lock().unwrap()[&id].load(Ordering::SeqCst));
        row
    };
    assert!(crate::build_record::start_review(&shared, &cancelling).is_err());
    drop(slot);
    idle(&runtime).await;
    let sessions = shared.lock().unwrap();
    let child = sessions.store.load_job(&id).unwrap().unwrap();
    let (_, result) = crate::review_record::decode(&child).unwrap();
    assert_eq!(result.phase, JobPhase::Cancelled);
    assert!(result.report.is_none());
    assert_eq!(decode(&cancelling).unwrap().1.phase, JobPhase::Cancelling);
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        cancelling
    );
    assert!(runtime.cancellations.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_dir(sessions.review_directory.as_ref().unwrap())
            .unwrap()
            .count(),
        0
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
