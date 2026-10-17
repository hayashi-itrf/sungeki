use super::*;
use std::sync::{Arc, Mutex};

async fn ready() -> (std::path::PathBuf, Arc<Mutex<Sessions>>, StoredJob) {
    let (root, shared, parent) = super::build_review_tests::ready().await;
    crate::build_record::start_review(&shared, &parent).unwrap();
    let runtime = shared.lock().unwrap().review_runtime.clone();
    super::build_review_tests::idle(&runtime).await;
    (root, shared, parent)
}
#[tokio::test]
async fn review_completion_preserves_uninspected_report_and_survives_replay_restart() {
    let (root, shared, parent) = ready().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    let output = crate::build_record::accept_review(&shared, &parent).unwrap();
    assert!(output.review.diagnostics.summary.not_run > 0);
    assert!(output.review.asset_fingerprint.is_some());
    let completed = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    let (input, state) = decode(&completed).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert_eq!(state.receipts.len(), input.step_ids.len());
    assert!(state.child_id.is_none());
    let mut corrupt = state.clone();
    corrupt.output.as_mut().unwrap().review.source_revision += 1;
    assert!(corrupt.validate(&input).is_err());
    let mut corrupt = state.clone();
    corrupt
        .output
        .as_mut()
        .unwrap()
        .review
        .diagnostics
        .summary
        .complete = true;
    assert!(corrupt.validate(&input).is_err());
    let sessions = shared.lock().unwrap();
    let child = sessions
        .store
        .load_job(&output.review_job_id)
        .unwrap()
        .unwrap();
    let report = crate::review_record::decode(&child)
        .unwrap()
        .1
        .report
        .unwrap();
    assert_eq!(
        serde_json::to_value(&output.review).unwrap(),
        serde_json::to_value(report).unwrap()
    );
    drop(sessions);
    assert_eq!(output.project_path, input.document_path);
    assert_eq!(output.review_job_id, output.review.report_id);
    for _ in 0..2 {
        let again = crate::build_record::accept_review(&shared, &completed).unwrap();
        assert_eq!(
            serde_json::to_value(&again).unwrap(),
            serde_json::to_value(&output).unwrap()
        );
    }
    assert!(crate::build_record::accept_review(&shared, &parent).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load("session")
            .unwrap()
            .unwrap(),
        before
    );
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        completed
    );
    crate::build_record::accept_review(&shared, &completed).unwrap();
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn mismatched_report_cancel_and_changed_original_do_not_complete_parent() {
    let (root, shared, parent) = ready().await;
    let (input, state) = decode(&parent).unwrap();
    let id = state.child_id.unwrap();
    let (child, mut result) = {
        let sessions = shared.lock().unwrap();
        let child = sessions.store.load_job(&id).unwrap().unwrap();
        let result = crate::review_record::decode(&child).unwrap().1;
        (child, result)
    };
    result.report.as_mut().unwrap().source_revision += 1;
    crate::review_jobs::save(&mut shared.lock().unwrap(), &child, &result).unwrap();
    assert!(crate::build_record::accept_review(&shared, &parent).is_err());
    result.report.as_mut().unwrap().source_revision -= 1;
    {
        let mut sessions = shared.lock().unwrap();
        let child = sessions.store.load_job(&id).unwrap().unwrap();
        crate::review_jobs::save(&mut sessions, &child, &result).unwrap();
    }
    let sungeki_project::AssetLocator::Absolute { path } = &input.recipe.assets[0].locator else {
        panic!()
    };
    let bytes = std::fs::read(path).unwrap();
    std::fs::write(path, vec![0; bytes.len()]).unwrap();
    assert!(crate::build_record::accept_review(&shared, &parent).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        parent
    );
    std::fs::write(path, bytes).unwrap();
    let cancelling =
        crate::build_record::request_cancel(&mut shared.lock().unwrap(), &parent).unwrap();
    assert!(crate::build_record::accept_review(&shared, &cancelling).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        cancelling
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
