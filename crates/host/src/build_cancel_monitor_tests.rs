use super::*;
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
async fn idle(shared: &Arc<Mutex<Sessions>>) {
    let active = shared.lock().unwrap().build_runtime.active.clone();
    tokio::time::timeout(Duration::from_secs(3), async {
        while active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn cancellation_monitor_waits_for_resources_and_does_not_duplicate() {
    let (root, mut sessions, parent, input, child) = super::speech_input_tests::setup();
    crate::speech_journal::create(
        &mut sessions,
        child.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    let lease = sessions
        .speech_resources
        .try_acquire(&child, vec![sungeki_jobs::Resource::Gpu])
        .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let cancelling = crate::build_record::cancel_owned(&shared, &parent).unwrap();
    assert!(
        crate::build_record::monitor_cancel(&shared, &parent.id, Duration::from_secs(2)).unwrap()
    );
    assert!(
        !crate::build_record::monitor_cancel(&shared, &parent.id, Duration::from_secs(2)).unwrap()
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
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
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .build_runtime
            .active
            .load(Ordering::SeqCst),
        1
    );
    drop(lease);
    idle(&shared).await;
    assert_eq!(
        decode(
            &shared
                .lock()
                .unwrap()
                .store
                .load_job(&parent.id)
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .1
        .phase,
        JobPhase::Cancelled
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn expired_monitor_keeps_uncertain_cancellation_and_can_be_observed_again() {
    let (root, sessions, parent, _, _) = super::speech_input_tests::setup();
    let shared = Arc::new(Mutex::new(sessions));
    crate::build_record::cancel_owned(&shared, &parent).unwrap();
    for _ in 0..2 {
        assert!(crate::build_record::monitor_cancel(&shared, &parent.id, Duration::ZERO).unwrap());
        idle(&shared).await;
        let row = shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap();
        let (_, state) = decode(&row).unwrap();
        assert_eq!(state.phase, JobPhase::Cancelling);
        assert!(state.child_id.is_some());
        assert!(state.progress.unwrap().stage.contains("停止確認未完了"));
    }
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
