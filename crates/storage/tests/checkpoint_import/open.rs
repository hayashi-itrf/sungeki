use super::*;
fn document(f: &Fixture, session_id: &str) -> (StoredSession, DocumentBinding) {
    (
        StoredSession {
            id: session_id.into(),
            revision: 0,
            version: 0,
            snapshot: b"validated editor snapshot fixture".to_vec().into(),
        },
        DocumentBinding {
            session_id: session_id.into(),
            path: f.root.join("project.vvm.json").to_string_lossy().into(),
            stamp: FileStamp {
                device: 1,
                inode: 2,
                hash: "a".repeat(64),
            },
            saved_content_hash: "b".repeat(64),
        },
    )
}
#[test]
fn successful_open_and_import_survive_restart_together() {
    let f = Fixture::new();
    let mut store = f.store();
    let (state, binding) = document(&f, "new-session");
    let result = store
        .create_document_session_with_checkpoints(
            &state,
            &binding,
            &f.portable.project.id,
            &[f.batch(&f.portable)],
        )
        .unwrap();
    assert_eq!(result.inserted, 1);
    drop(store);
    let store = f.store();
    assert_eq!(store.load(&state.id).unwrap(), Some(state));
    assert_eq!(
        store.document_binding(&binding.session_id).unwrap(),
        Some(binding)
    );
    let c = store.checkpoint(&f.portable.id).unwrap().unwrap();
    assert_eq!(c.source_session_id, "new-session");
    assert_eq!(
        store.imported_checkpoint_origin(&f.portable.id).unwrap(),
        Some(f.portable.origin.clone())
    );
}
#[test]
fn late_checkpoint_conflict_rolls_back_document_session_and_earlier_imports() {
    let f = Fixture::new();
    let mut store = f.store();
    let pid = &f.portable.project.id;
    store
        .create_checkpoint(&StoredCheckpoint {
            source_session_id: "local".into(),
            checkpoint: f.portable.resolve(&f.root, pid).unwrap(),
        })
        .unwrap();
    let mut first = f.portable.clone();
    first.id = id();
    let (state, binding) = document(&f, "new-session");
    assert!(matches!(
        store.create_document_session_with_checkpoints(
            &state,
            &binding,
            pid,
            &[f.batch(&first), f.batch(&f.portable)]
        ),
        Err(StorageError::Conflict)
    ));
    assert!(store.load(&state.id).unwrap().is_none());
    assert!(store.document_binding(&state.id).unwrap().is_none());
    assert!(
        store
            .document_owner(&binding.path, Some(&binding.stamp))
            .unwrap()
            .is_none()
    );
    assert!(store.checkpoint(&first.id).unwrap().is_none());
    assert!(
        store
            .imported_checkpoint_origin(&first.id)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .checkpoint(&f.portable.id)
            .unwrap()
            .unwrap()
            .source_session_id,
        "local"
    );
}
#[test]
fn occupied_document_or_invalid_state_does_not_import_any_history() {
    let f = Fixture::new();
    let mut store = f.store();
    let pid = &f.portable.project.id;
    let (state, binding) = document(&f, "original");
    store.create_document_session(&state, &binding).unwrap();
    let (mut next, other) = document(&f, "other");
    assert!(matches!(
        store.create_document_session_with_checkpoints(&next, &other, pid, &[f.batch(&f.portable)]),
        Err(StorageError::Conflict)
    ));
    next.revision = 1;
    assert!(
        store
            .create_document_session_with_checkpoints(&next, &other, pid, &[f.batch(&f.portable)])
            .is_err()
    );
    assert!(store.load(&next.id).unwrap().is_none());
    assert!(store.checkpoint(&f.portable.id).unwrap().is_none());
    assert_eq!(store.document_binding(&state.id).unwrap(), Some(binding));
}
