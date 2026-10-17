use super::*;
use std::sync::{Arc, Mutex};
#[test]
fn parent_cancel_reaches_unsent_speech_and_replays_without_claiming_parent_settled() {
    let (root, mut sessions, parent, input, child_id) = super::speech_input_tests::setup();
    crate::speech_journal::create(
        &mut sessions,
        child_id.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let cancelling = crate::build_record::cancel_owned(&shared, &parent).unwrap();
    assert_eq!(decode(&cancelling).unwrap().1.phase, JobPhase::Cancelling);
    let child = shared
        .lock()
        .unwrap()
        .store
        .load_job(&child_id)
        .unwrap()
        .unwrap();
    let (_, state) = crate::speech_journal::decode(&child).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelled);
    assert_eq!(
        state.execution,
        crate::speech_state::Execution::NotDispatched
    );
    assert_eq!(
        crate::build_record::cancel_owned(&shared, &cancelling).unwrap(),
        cancelling
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&child_id)
            .unwrap()
            .unwrap(),
        child
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
