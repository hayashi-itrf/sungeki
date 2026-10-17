use super::*;
use crate::build_record::{Reservation, reserve};

pub(super) async fn ready() -> (
    std::path::PathBuf,
    std::sync::Arc<std::sync::Mutex<Sessions>>,
    StoredJob,
) {
    let (root, shared, row) = super::layout_apply_tests::ready().await;
    let row = {
        let mut sessions = shared.lock().unwrap();
        crate::build_record::layout(&mut sessions, &row).unwrap();
        let row = sessions.store.load_job(&row.id).unwrap().unwrap();
        let Reservation::New(row) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        row
    };
    (root, shared, row)
}
#[tokio::test]
async fn preflight_checks_actual_sources_and_persists_observation_without_editing() {
    let (root, shared, row) = ready().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    let proof = crate::build_record::validate_output(&shared, &row)
        .await
        .unwrap();
    assert_eq!(proof.revision, before.revision);
    assert_eq!(proof.checked_sources, 3);
    assert!(proof.frame_count > 0);
    let checked = shared
        .lock()
        .unwrap()
        .store
        .load_job(&row.id)
        .unwrap()
        .unwrap();
    let (_, state) = decode(&checked).unwrap();
    assert_eq!(state.receipts.len(), 6);
    assert!(state.child_id.is_none());
    assert_eq!(state.preflight.unwrap().content_hash, proof.content_hash);
    let replay = crate::build_record::validate_output(&shared, &checked)
        .await
        .unwrap();
    assert_eq!(replay.content_hash, proof.content_hash);
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
    let sessions = Sessions::open(&root.join("db")).unwrap();
    let (_, state) = decode(&sessions.store.load_job(&row.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.preflight.unwrap().content_hash, proof.content_hash);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn missing_changed_or_linked_source_never_advances_validation() {
    for mode in 0..3 {
        let (root, shared, row) = ready().await;
        let source = root.join("source.png");
        match mode {
            0 => std::fs::remove_file(&source).unwrap(),
            1 => {
                let mut bytes = std::fs::read(&source).unwrap();
                bytes[20] ^= 1;
                std::fs::write(&source, bytes).unwrap();
            }
            _ => {
                let target = root.join("other.png");
                std::fs::rename(&source, &target).unwrap();
                std::os::unix::fs::symlink(target, &source).unwrap();
            }
        }
        let before = shared
            .lock()
            .unwrap()
            .store
            .load("session")
            .unwrap()
            .unwrap();
        assert!(
            crate::build_record::validate_output(&shared, &row)
                .await
                .is_err()
        );
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), row);
        assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
        drop(sessions);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}
