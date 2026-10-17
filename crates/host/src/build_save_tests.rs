use super::*;
use crate::build_record::{Reservation, reserve};

pub(super) async fn ready() -> (
    std::path::PathBuf,
    std::sync::Arc<std::sync::Mutex<Sessions>>,
    StoredJob,
) {
    let (root, shared, row) = super::bake_tests::ready().await;
    let row = {
        let mut sessions = shared.lock().unwrap();
        crate::build_record::finish_bake(&mut sessions, &row).unwrap();
        let row = sessions.store.load_job(&row.id).unwrap().unwrap();
        let Reservation::New(row) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        row
    };
    (root, shared, row)
}
#[tokio::test]
async fn fixed_save_replays_without_rewriting_and_survives_restart() {
    let (root, shared, row) = ready().await;
    let path = decode(&row).unwrap().0.document_path;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    let saved = crate::build_record::save_project(&shared, &row).unwrap();
    let loaded = sungeki_storage::load_document(&path).unwrap();
    assert_eq!(saved.stamp, loaded.stamp);
    assert_eq!(
        saved.content_hash,
        sungeki_storage::project_hash(&loaded.project).unwrap()
    );
    let current = shared
        .lock()
        .unwrap()
        .store
        .load_job(&row.id)
        .unwrap()
        .unwrap();
    assert_eq!(decode(&current).unwrap().1.receipts.len(), 8);
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
    let replay = crate::build_record::save_project(&shared, &current).unwrap();
    assert_eq!(replay.stamp, saved.stamp);
    drop(shared);
    let shared = std::sync::Arc::new(std::sync::Mutex::new(
        Sessions::open(&root.join("db")).unwrap(),
    ));
    let recovered = shared
        .lock()
        .unwrap()
        .store
        .load_job(&row.id)
        .unwrap()
        .unwrap();
    assert_eq!(decode(&recovered).unwrap().1.phase, JobPhase::Interrupted);
    assert_eq!(
        crate::build_record::save_project(&shared, &recovered)
            .unwrap()
            .stamp,
        saved.stamp
    );
    std::fs::write(&path, b"external edit").unwrap();
    assert!(crate::build_record::save_project(&shared, &recovered).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"external edit");
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn existing_file_is_never_overwritten_and_parent_does_not_advance() {
    let (root, shared, row) = ready().await;
    let path = decode(&row).unwrap().0.document_path;
    std::fs::write(&path, b"existing work").unwrap();
    assert!(crate::build_record::save_project(&shared, &row).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"existing work");
    let sessions = shared.lock().unwrap();
    assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), row);
    assert!(
        sessions
            .store
            .document_binding("session")
            .unwrap()
            .is_none()
    );
    assert!(sessions.store.save_intents().unwrap().is_empty());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn completed_file_with_failed_parent_receipt_is_reconciled_without_rewrite() {
    let (root, shared, row) = ready().await;
    let raw = rusqlite::Connection::open(root.join("db")).unwrap();
    raw.execute_batch("CREATE TRIGGER reject_parent BEFORE UPDATE ON jobs WHEN OLD.kind='build_video' BEGIN SELECT RAISE(ABORT,'simulated parent receipt failure'); END;").unwrap();
    assert!(crate::build_record::save_project(&shared, &row).is_err());
    let binding = shared
        .lock()
        .unwrap()
        .store
        .document_binding("session")
        .unwrap()
        .unwrap();
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&row.id)
            .unwrap()
            .unwrap(),
        row
    );
    raw.execute_batch("DROP TRIGGER reject_parent;").unwrap();
    let saved = crate::build_record::save_project(&shared, &row).unwrap();
    assert_eq!(saved.stamp, binding.stamp);
    let current = shared
        .lock()
        .unwrap()
        .store
        .load_job(&row.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        decode(&current).unwrap().1.saved.unwrap().stamp,
        binding.stamp
    );
    drop(raw);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
