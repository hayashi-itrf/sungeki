use super::*;
use std::sync::{Arc, Mutex, atomic::Ordering};

#[tokio::test]
async fn one_owner_advances_until_missing_engine_records_interruption_and_does_not_resume() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let mut recipe = decode(&fixture).unwrap().0.recipe;
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    let parent = crate::build_record::create(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe.clone(),
        &root.join("連続実行.vvm.json"),
    )
    .unwrap();
    let other = crate::build_record::create(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("別の制作.vvm.json"),
    )
    .unwrap();
    let runtime = sessions.build_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    assert!(crate::build_record::launch(&shared, &parent.id, None).unwrap());
    assert!(!crate::build_record::launch(&shared, &parent.id, None).unwrap());
    assert!(crate::build_record::launch(&shared, &other.id, None).is_err());
    assert_eq!(runtime.active.load(Ordering::SeqCst), 1);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
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
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.receipts.len(), 2);
    assert!(state.child_id.is_none());
    assert_eq!(
        state.error.unwrap().code,
        sungeki_protocol::ErrorCode::CapabilityUnavailable
    );
    assert!(crate::build_record::launch(&shared, &parent.id, None).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&other.id)
            .unwrap()
            .unwrap(),
        other
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .speech_runtime
            .active
            .load(Ordering::SeqCst),
        0
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
        row
    );
    assert!(crate::build_record::launch(&shared, &parent.id, None).is_err());
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
