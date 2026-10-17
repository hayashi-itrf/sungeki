use sungeki_storage::*;

#[test]
fn build_job_migration_replay_cas_and_reopen_preserve_existing_work() {
    let root = std::env::temp_dir().join(format!("vvm-build-store-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("recovery.sqlite");
    let mut store = RecoveryStore::open(&path).unwrap();
    store
        .commit(
            None,
            &StoredSession {
                id: "session".into(),
                revision: 0,
                version: 0,
                snapshot: b"untouched project".to_vec().into(),
            },
        )
        .unwrap();
    let existing: Vec<_> = [
        JobKind::Review,
        JobKind::Speech,
        JobKind::Render,
        JobKind::Loudness,
        JobKind::AudioPreview,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| {
        let job = StoredJob {
            id: format!("old-{i}"),
            kind,
            session_id: "session".into(),
            input: vec![i as u8],
            version: 0,
            state: vec![i as u8, 5],
        };
        store.create_job(&job).unwrap();
        job
    })
    .collect();
    drop(store);
    // Recreate the actual preceding Sungeki schema, with all five job kinds.
    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute_batch(
        "CREATE TABLE jobs_v10 (
        id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
        input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
        kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview')));
        INSERT INTO jobs_v10 SELECT id,session_id,input,version,state,kind FROM jobs;
        UPDATE sessions SET snapshot=CAST('untouched project' AS BLOB); DROP TABLE jobs; ALTER TABLE jobs_v10 RENAME TO jobs; PRAGMA user_version=10;",
    )
    .unwrap();
    drop(raw);
    let mut store = RecoveryStore::open(&path).unwrap();
    for old in &existing {
        assert_eq!(store.load_job(&old.id).unwrap().as_ref(), Some(old));
    }
    let parent = StoredJob {
        id: "parent".into(),
        kind: JobKind::BuildVideo,
        session_id: "session".into(),
        input: b"fixed recipe and child ids".to_vec(),
        version: 0,
        state: br#"{"completed":0,"child_id":null}"#.to_vec(),
    };
    store.create_job(&parent).unwrap();
    let reserved = br#"{"completed":0,"child_id":"prepared-child"}"#;
    assert_eq!(
        store
            .update_job("parent", JobKind::BuildVideo, 0, reserved)
            .unwrap(),
        1
    );
    let replay = store.create_job(&parent).unwrap();
    assert_eq!(replay.state, reserved);
    assert_eq!(replay.version, 1);
    assert!(
        store
            .update_job("parent", JobKind::BuildVideo, 0, b"late update")
            .is_err()
    );
    assert!(
        store
            .update_job("parent", JobKind::Render, 1, b"wrong kind")
            .is_err()
    );
    assert!(
        store
            .create_job(&StoredJob {
                input: b"different".to_vec(),
                ..parent.clone()
            })
            .is_err()
    );
    assert!(
        store
            .create_job(&StoredJob {
                kind: JobKind::Render,
                ..parent.clone()
            })
            .is_err()
    );
    drop(store);
    let store = RecoveryStore::open(&path).unwrap();
    assert_eq!(store.load_job("parent").unwrap().unwrap(), replay);
    assert_eq!(
        store
            .list_jobs(Some(JobKind::BuildVideo), None, None, 100)
            .unwrap(),
        vec!["parent"]
    );
    for old in &existing {
        assert_eq!(store.load_job(&old.id).unwrap().as_ref(), Some(old));
    }
    assert_eq!(
        store.load("session").unwrap().unwrap().snapshot.as_slice(),
        b"untouched project"
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
