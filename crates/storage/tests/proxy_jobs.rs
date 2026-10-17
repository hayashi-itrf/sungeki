use rusqlite::Connection;
use std::fs;
use sungeki_storage::{JobKind, RecoveryStore, StorageError, StoredJob};

#[test]
fn v15_upgrade_preserves_existing_records_and_proxy_cas_replay() {
    let root = std::env::temp_dir().join(format!("proxy-db-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("db");
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,revision INTEGER,version INTEGER,snapshot BLOB);
        INSERT INTO sessions VALUES('s',0,0,X'00');
        CREATE TABLE jobs(id TEXT PRIMARY KEY,session_id TEXT,input BLOB,version INTEGER,state BLOB,
        kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview','build_video','frame_preview','collection')));
        PRAGMA user_version=15;").unwrap();
    let kinds = [
        JobKind::Review,
        JobKind::Speech,
        JobKind::Render,
        JobKind::Loudness,
        JobKind::AudioPreview,
        JobKind::BuildVideo,
        JobKind::FramePreview,
        JobKind::Collection,
    ];
    for (i, kind) in kinds.iter().enumerate() {
        raw.execute(
            "INSERT INTO jobs VALUES(?1,'s',X'0123',4,X'4567',?2)",
            rusqlite::params![format!("old-{i}"), kind],
        )
        .unwrap();
    }
    drop(raw);
    let mut store = RecoveryStore::open(&path).unwrap();
    for (i, kind) in kinds.iter().enumerate() {
        let record = store.load_job(&format!("old-{i}")).unwrap().unwrap();
        assert_eq!(
            (record.kind, record.version, record.input, record.state),
            (*kind, 4, vec![1, 35], vec![69, 103])
        );
    }
    let job = StoredJob {
        id: "proxy-job".into(),
        session_id: "s".into(),
        kind: JobKind::Proxy,
        version: 0,
        input: b"immutable proxy request".to_vec(),
        state: b"queued".to_vec(),
    };
    store.create_job(&job).unwrap();
    store
        .update_job(&job.id, JobKind::Proxy, 0, b"running")
        .unwrap();
    assert!(matches!(
        store.update_job(&job.id, JobKind::Proxy, 0, b"stale"),
        Err(StorageError::Conflict)
    ));
    assert!(matches!(
        store.create_job(&StoredJob {
            input: b"other".to_vec(),
            ..job.clone()
        }),
        Err(StorageError::Conflict)
    ));
    assert!(matches!(
        store.create_job(&StoredJob {
            kind: JobKind::Render,
            ..job.clone()
        }),
        Err(StorageError::Conflict)
    ));
    drop(store);
    let mut store = RecoveryStore::open(&path).unwrap();
    let replay = store.create_job(&job).unwrap();
    assert_eq!((replay.version, replay.state), (1, b"running".to_vec()));
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Proxy), Some("s"), None, 10)
            .unwrap(),
        vec!["proxy-job"]
    );
    drop(store);
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        18
    );
    fs::remove_dir_all(root).unwrap();
}
