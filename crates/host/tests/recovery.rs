use sungeki_editor::*;
use sungeki_project::Project;
use sungeki_storage::*;
#[test]
fn committed_edit_and_receipt_restore_and_stale_write_is_rejected() {
    let directory = std::env::temp_dir().join(format!("vvm-storage-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("recovery.sqlite");
    let id = uuid::Uuid::new_v4().to_string();
    let mut session = EditSession::new(Project::empty(id.clone(), "before".into())).unwrap();
    let request = EditRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: 0,
        action: EditAction::Apply {
            commands: vec![Command::RenameProject {
                title: "after".into(),
            }],
        },
    };
    {
        let mut store = RecoveryStore::open(&path).unwrap();
        store
            .commit(
                None,
                &StoredSession {
                    id: id.clone(),
                    revision: 0,
                    version: 0,
                    snapshot: session.snapshot().unwrap().into(),
                },
            )
            .unwrap();
        session.execute(request.clone()).unwrap();
        let state = StoredSession {
            id: id.clone(),
            revision: 1,
            version: 1,
            snapshot: session.snapshot().unwrap().into(),
        };
        store.commit(Some(0), &state).unwrap();
        assert!(matches!(
            store.commit(Some(0), &state),
            Err(StorageError::Conflict)
        ));
    }
    {
        let store = RecoveryStore::open(&path).unwrap();
        assert_eq!(store.list().unwrap(), vec![id.clone()]);
        let stored = store.load(&id).unwrap().unwrap();
        let mut restored = EditSession::restore(&stored.snapshot).unwrap();
        let receipt = restored.execute(request).unwrap();
        assert_eq!(receipt.revision, 1);
        assert_eq!(restored.history().project().title, "after");
        assert_eq!(restored.history().undo_len(), 1);
    }
    std::fs::remove_dir_all(directory).unwrap();
}
