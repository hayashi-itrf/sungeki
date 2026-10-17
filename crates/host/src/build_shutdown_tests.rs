use super::*;
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

#[tokio::test]
async fn shutdown_cancels_queued_owned_parent_and_blocks_new_admission() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let mut recipe = decode(&fixture).unwrap().0.recipe;
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    let parent = crate::build_record::create(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe.clone(),
        &root.join("終了検証.vvm.json"),
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    crate::build_record::launch(&shared, &parent.id, None).unwrap();
    crate::build_record::shutdown(&shared, Duration::from_secs(3))
        .await
        .unwrap();
    let mut sessions = shared.lock().unwrap();
    assert_eq!(sessions.build_runtime.active.load(Ordering::SeqCst), 0);
    assert!(sessions.build_runtime.stopping.load(Ordering::SeqCst));
    for id in [&parent.id, &fixture.id] {
        let (_, state) = decode(&sessions.store.load_job(id).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, JobPhase::Cancelled);
        assert!(state.receipts.is_empty());
    }
    assert!(
        crate::build_record::create(
            &mut sessions,
            &uuid::Uuid::new_v4().to_string(),
            recipe,
            &root.join("遅い要求.vvm.json")
        )
        .is_err()
    );
    drop(sessions);
    assert!(crate::build_record::launch(&shared, &parent.id, None).is_err());
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn shutdown_timeout_preserves_missing_child_and_restart_does_not_claim_cleanup() {
    let (root, sessions, parent, _, child) = super::speech_input_tests::setup();
    let shared = Arc::new(Mutex::new(sessions));
    assert!(
        crate::build_record::shutdown(&shared, Duration::from_millis(20))
            .await
            .is_err()
    );
    let active = shared.lock().unwrap().build_runtime.active.clone();
    tokio::time::timeout(Duration::from_secs(3), async {
        while active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let row = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    let (_, state) = decode(&row).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelling);
    assert_eq!(state.child_id.as_deref(), Some(child.as_str()));
    assert!(state.progress.unwrap().stage.contains("停止確認未完了"));
    drop(shared);
    let sessions = Sessions::open(&root.join("db")).unwrap();
    let restored = sessions.store.load_job(&parent.id).unwrap().unwrap();
    assert_eq!(decode(&restored).unwrap().1.phase, JobPhase::Interrupted);
    assert_eq!(
        decode(&restored).unwrap().1.child_id.as_deref(),
        Some(child.as_str())
    );
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn shutdown_waits_for_review_worker_cleanup_before_completing_parent() {
    let (root, shared, parent) = super::build_review_tests::ready().await;
    let runtime = shared.lock().unwrap().review_runtime.clone();
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    crate::build_record::start_review(&shared, &parent).unwrap();
    let owned = shared.clone();
    let shutdown =
        tokio::spawn(
            async move { crate::build_record::shutdown(&owned, Duration::from_secs(3)).await },
        );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let row = shared
                .lock()
                .unwrap()
                .store
                .load_job(&parent.id)
                .unwrap()
                .unwrap();
            if decode(&row).unwrap().1.phase == JobPhase::Cancelling {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!shutdown.is_finished());
    assert_eq!(runtime.active.load(Ordering::SeqCst), 1);
    drop(slot);
    shutdown.await.unwrap().unwrap();
    let sessions = shared.lock().unwrap();
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    assert_eq!(sessions.build_runtime.active.load(Ordering::SeqCst), 0);
    assert_eq!(
        decode(&sessions.store.load_job(&parent.id).unwrap().unwrap())
            .unwrap()
            .1
            .phase,
        JobPhase::Cancelled
    );
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
