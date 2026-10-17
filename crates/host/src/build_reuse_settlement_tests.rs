use super::*;
use crate::build_record::{settlement, speech_reuse::verify};

#[tokio::test]
async fn another_speech_resource_blocks_reuse_until_settlement_without_changing_records() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let (row, mut lease) = {
        let sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent).unwrap().unwrap();
        let (plan, _) = decode(&row).unwrap();
        let lease = sessions
            .speech_resources
            .try_acquire(&plan.step_ids[3], vec![sungeki_jobs::Resource::Gpu])
            .unwrap();
        (row, lease)
    };
    assert!(verify(&shared, &parent, &child, &target).await.is_err());
    lease.start().unwrap();
    assert!(verify(&shared, &parent, &child, &target).await.is_err());
    drop(lease);
    assert!(verify(&shared, &parent, &child, &target).await.is_err());
    {
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.store.load_job(&parent).unwrap().unwrap(), row);
        // No external process exists in this fixture. Explicitly settle its lease.
        let record = sessions.speech_resources.quarantined().unwrap().remove(0);
        sessions.speech_resources.confirm_settled(&record).unwrap();
        assert!(settlement::ready(&sessions, &row).unwrap());
    }
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    assert_eq!(proof.parent, row);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn unknown_other_child_dispatch_blocks_reuse_and_stale_parent_is_rejected() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let stale = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent).unwrap().unwrap();
        let (plan, mut state) = decode(&row).unwrap();
        state.child_id = Some(plan.step_ids[3].clone());
        state.validate(&plan).unwrap();
        sessions
            .store
            .update_job(
                &parent,
                JobKind::BuildVideo,
                row.version,
                &serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
        row
    };
    assert!(verify(&shared, &parent, &child, &target).await.is_err());
    let sessions = shared.lock().unwrap();
    assert!(settlement::ready(&sessions, &stale).is_err());
    let current = sessions.store.load_job(&parent).unwrap().unwrap();
    assert!(!settlement::ready(&sessions, &current).unwrap());
    assert_eq!(sessions.store.load_job(&parent).unwrap().unwrap(), current);
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
