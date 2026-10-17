use super::*;
use crate::build_record::{self as build, continuation::Continuation, speech_reuse::verify};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

async fn setup(change_text: bool) -> (std::path::PathBuf, Arc<Mutex<Sessions>>, StoredJob) {
    let (root, shared, source, child, target) = super::speech_reuse_tests::setup().await;
    let proof = verify(&shared, &source, &child, &target).await.unwrap();
    let source = Continuation::from_verified(&[(0, proof)], 2).unwrap();
    let row = {
        let mut sessions = shared.lock().unwrap();
        let mut recipe = input().recipe;
        let source_id = source.source_for(0).unwrap().0;
        recipe.assets = decode(&sessions.store.load_job(source_id).unwrap().unwrap())
            .unwrap()
            .0
            .recipe
            .assets;
        for role in &mut recipe.roles {
            role.preset_id = "image-custom-v1".into();
        }
        if change_text {
            recipe.scenes[0].lines[0].spoken_text = Some("変更した発音文".into());
        }
        build::create_continued(
            &mut sessions,
            &uuid::Uuid::new_v4().to_string(),
            recipe,
            &root.join("再利用する実行工程.vvm.json"),
            None,
            Default::default(),
            Some(source),
        )
        .unwrap()
    };
    (root, shared, row)
}

#[tokio::test]
async fn dispatcher_reuses_without_engine_then_requires_engine_for_the_unfinished_line() {
    let (root, shared, row) = setup(false).await;
    // Production startup defers original-directory initialization until use.
    shared.lock().unwrap().speech_directory = Err(sungeki_protocol::ApiError::new(
        sungeki_protocol::ErrorCode::CapabilityUnavailable,
        "not initialized",
    ));
    assert!(build::launch(&shared, &row.id, None).unwrap());
    let runtime = shared.lock().unwrap().build_runtime.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let sessions = shared.lock().unwrap();
    let current = sessions.store.load_job(&row.id).unwrap().unwrap();
    let (plan, state) = decode(&current).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.receipts.len(), 3); // Prepare, Compose, reused speech.
    assert!(state.child_id.is_none());
    assert_eq!(
        state.error.unwrap().code,
        sungeki_protocol::ErrorCode::CapabilityUnavailable
    );
    let child = sessions.store.load_job(&plan.step_ids[2]).unwrap().unwrap();
    let (_, reused) = crate::speech_journal::decode_input(&child).unwrap();
    assert_eq!(reused.execution, crate::speech_state::Execution::Reused);
    assert!(matches!(
        reused.application,
        crate::speech_state::Application::Applied { .. }
    ));
    assert!(
        sessions
            .store
            .load_job(&plan.step_ids[3])
            .unwrap()
            .is_none()
    );
    assert_eq!(sessions.speech_runtime.active.load(Ordering::SeqCst), 0);
    assert!(!sessions.speech_resources.has_owner(&child.id).unwrap());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn changed_target_stops_without_silent_resynthesis_or_a_successful_child() {
    let (root, shared, row) = setup(true).await;
    build::launch(&shared, &row.id, None).unwrap();
    let runtime = shared.lock().unwrap().build_runtime.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let sessions = shared.lock().unwrap();
    let (plan, state) = decode(&sessions.store.load_job(&row.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.receipts.len(), 2);
    assert!(state.child_id.is_none());
    assert!(state.error.unwrap().message.contains("再合成が必要"));
    assert!(
        sessions
            .store
            .load_job(&plan.step_ids[2])
            .unwrap()
            .is_none()
    );
    assert!(
        !sessions
            .speech_directory
            .as_ref()
            .unwrap()
            .join(&plan.step_ids[2])
            .exists()
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancelled_local_reuse_releases_unstarted_reservation() {
    let (root, shared, row) = setup(false).await;
    for _ in 0..2 {
        assert_eq!(
            build::advance(&shared, &row.id, None).await.unwrap(),
            build::Advance::Progress
        );
    }
    let reserved = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&row.id).unwrap().unwrap();
        let build::Reservation::New(reserved) = build::reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        reserved
    };
    build::cancel_owned(&shared, &reserved).unwrap();
    assert!(build::reuse_runtime::run(&shared, &reserved).await.is_err());
    let mut sessions = shared.lock().unwrap();
    let current = sessions.store.load_job(&row.id).unwrap().unwrap();
    assert!(decode(&current).unwrap().1.child_id.is_none());
    assert!(build::settle_cancel(&mut sessions, &current).unwrap());
    assert_eq!(
        decode(&sessions.store.load_job(&row.id).unwrap().unwrap())
            .unwrap()
            .1
            .phase,
        JobPhase::Cancelled
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
