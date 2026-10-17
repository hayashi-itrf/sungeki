use sungeki_storage::*;

#[test]
fn creation_rolls_back_on_parent_failure_and_never_replaces_existing_session() {
    let root = std::env::temp_dir().join(format!("vvm-build-create-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("db");
    let mut store = RecoveryStore::open(&path).unwrap();
    let session = StoredSession {
        id: "new".into(),
        revision: 0,
        version: 0,
        snapshot: b"empty project".to_vec().into(),
    };
    let parent = StoredJob {
        id: "parent".into(),
        kind: JobKind::BuildVideo,
        session_id: session.id.clone(),
        version: 0,
        input: b"fixed plan".to_vec(),
        state: b"queued".to_vec(),
    };
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_build BEFORE INSERT ON jobs BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    assert!(store.create_build(&session, &parent).is_err());
    assert!(store.load(&session.id).unwrap().is_none());
    assert!(store.load_job(&parent.id).unwrap().is_none());
    conn.execute_batch("DROP TRIGGER fail_build;").unwrap();
    store.create_build(&session, &parent).unwrap();
    assert!(store.create_build(&session, &parent).is_err());
    let other = StoredSession {
        id: "other".into(),
        revision: 0,
        version: 0,
        snapshot: vec![].into(),
    };
    let duplicate = StoredJob {
        session_id: other.id.clone(),
        ..parent.clone()
    };
    assert!(store.create_build(&other, &duplicate).is_err());
    assert!(store.load(&other.id).unwrap().is_none());
    drop(store);
    let store = RecoveryStore::open(&path).unwrap();
    assert_eq!(store.load(&session.id).unwrap().unwrap(), session);
    assert_eq!(store.load_job(&parent.id).unwrap().unwrap(), parent);
    drop(store);
    drop(conn);
    std::fs::remove_dir_all(root).unwrap();
}
