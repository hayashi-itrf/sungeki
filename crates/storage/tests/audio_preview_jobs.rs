use sungeki_storage::*;
#[test]
fn v9_migration_preserves_existing_jobs_and_isolates_audio_preview() {
    let path =
        std::env::temp_dir().join(format!("preview-migration-{}.sqlite", uuid::Uuid::new_v4()));
    let mut store = RecoveryStore::open(&path).unwrap();
    store
        .commit(
            None,
            &StoredSession {
                id: "session".into(),
                revision: 0,
                version: 0,
                snapshot: b"unchanged".to_vec().into(),
            },
        )
        .unwrap();
    let original = StoredJob {
        id: "existing".into(),
        session_id: "session".into(),
        kind: JobKind::Loudness,
        input: b"original input".to_vec(),
        version: 0,
        state: b"original state".to_vec(),
    };
    store.create_job(&original).unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute_batch(
        "CREATE TABLE jobs_v9 (id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL,
        input BLOB NOT NULL, version INTEGER NOT NULL, state BLOB NOT NULL,
        kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness')));
        INSERT INTO jobs_v9 SELECT id,session_id,input,version,state,kind FROM jobs;
        UPDATE sessions SET snapshot=CAST('unchanged' AS BLOB); DROP TABLE jobs; ALTER TABLE jobs_v9 RENAME TO jobs; PRAGMA user_version=9;",
    )
    .unwrap();
    drop(raw);
    let mut store = RecoveryStore::open(&path).unwrap();
    assert_eq!(store.load_job("existing").unwrap().unwrap(), original);
    let preview = StoredJob {
        id: "preview".into(),
        kind: JobKind::AudioPreview,
        ..original.clone()
    };
    store.create_job(&preview).unwrap();
    assert_eq!(
        store
            .list_jobs(Some(JobKind::AudioPreview), None, None, 10)
            .unwrap(),
        vec!["preview"]
    );
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Loudness), None, None, 10)
            .unwrap(),
        vec!["existing"]
    );
    drop(store);
    let store = RecoveryStore::open(&path).unwrap();
    assert_eq!(store.load_job("preview").unwrap().unwrap(), preview);
    assert_eq!(
        store.load("session").unwrap().unwrap().snapshot.as_slice(),
        b"unchanged"
    );
    drop(store);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
