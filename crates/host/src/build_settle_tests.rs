use super::*;
use std::sync::{Arc, Mutex};
#[test]
fn cancelled_child_does_not_settle_parent_while_reserved_running_or_quarantined() {
    let (root, mut sessions, parent, input, child_id) = super::speech_input_tests::setup();
    crate::speech_journal::create(
        &mut sessions,
        child_id.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let parent = crate::build_record::cancel_owned(&shared, &parent).unwrap();
    let mut sessions = shared.lock().unwrap();
    let mut lease = sessions
        .speech_resources
        .try_acquire(&child_id, vec![sungeki_jobs::Resource::Gpu])
        .unwrap();
    assert!(!crate::build_record::settle_cancel(&mut sessions, &parent).unwrap());
    lease.start().unwrap();
    assert!(!crate::build_record::settle_cancel(&mut sessions, &parent).unwrap());
    drop(lease);
    assert!(!crate::build_record::settle_cancel(&mut sessions, &parent).unwrap());
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        parent
    );
    // This lease is a test fixture: no process or HTTP request was started.
    let settlement = sessions.speech_resources.quarantined().unwrap().remove(0);
    sessions
        .speech_resources
        .confirm_settled(&settlement)
        .unwrap();
    assert!(crate::build_record::settle_cancel(&mut sessions, &parent).unwrap());
    let settled = sessions.store.load_job(&parent.id).unwrap().unwrap();
    let (_, state) = decode(&settled).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelled);
    assert!(state.child_id.is_none());
    assert_eq!(state.receipts.len(), 2);
    assert!(crate::build_record::settle_cancel(&mut sessions, &settled).unwrap());
    drop(sessions);
    drop(shared);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    assert_eq!(
        sessions.store.load_job(&settled.id).unwrap().unwrap(),
        settled
    );
    assert!(crate::build_record::settle_cancel(&mut sessions, &settled).unwrap());
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_external_child_stays_uncertain_but_no_child_preparation_can_settle() {
    let (root, sessions, parent, _, _) = super::speech_input_tests::setup();
    let shared = Arc::new(Mutex::new(sessions));
    let parent = crate::build_record::cancel_owned(&shared, &parent).unwrap();
    assert!(!crate::build_record::settle_cancel(&mut shared.lock().unwrap(), &parent).unwrap());
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
    let (root, sessions, parent) = super::reservation_tests::setup(false);
    let shared = Arc::new(Mutex::new(sessions));
    let parent = crate::build_record::cancel_owned(&shared, &parent).unwrap();
    assert!(crate::build_record::settle_cancel(&mut shared.lock().unwrap(), &parent).unwrap());
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
