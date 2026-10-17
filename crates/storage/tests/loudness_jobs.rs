use std::{fs, path::PathBuf};
use sungeki_storage::{JobKind, RecoveryStore, StorageError, StoredJob};
struct Database(PathBuf);
impl Database {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("vvm-loudness-db-{}", uuid::Uuid::new_v4())))
    }
    fn seed_v8(&self) -> rusqlite::Connection {
        let raw = rusqlite::Connection::open(&self.0).unwrap();
        raw.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,version INTEGER NOT NULL,snapshot BLOB NOT NULL);
            INSERT INTO sessions VALUES('session',4,7,X'001122');
            CREATE TABLE jobs(id TEXT PRIMARY KEY,session_id TEXT NOT NULL,input BLOB NOT NULL,version INTEGER NOT NULL,state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render')));
            INSERT INTO jobs VALUES('review','session',X'01',3,X'02','review');
            INSERT INTO jobs VALUES('speech','session',X'03',5,X'04','speech');
            INSERT INTO jobs VALUES('render','session',X'05',9,X'06','render');
            CREATE INDEX jobs_session ON jobs(session_id,id);
            CREATE INDEX jobs_kind_session ON jobs(kind,session_id,id); PRAGMA user_version=8;").unwrap();
        raw
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(format!("{}{suffix}", self.0.display()));
        }
    }
}
#[test]
fn v8_upgrade_preserves_jobs_and_isolates_loudness_replays_and_cancellation() {
    let db = Database::new();
    drop(db.seed_v8());
    let mut store = RecoveryStore::open(&db.0).unwrap();
    let session = store.load("session").unwrap().unwrap();
    assert_eq!(
        (
            session.revision,
            session.version,
            session.snapshot.as_slice()
        ),
        (4, 7, [0, 17, 34].as_slice())
    );
    for (id, kind, input, version, state) in [
        ("review", JobKind::Review, 1, 3, 2),
        ("speech", JobKind::Speech, 3, 5, 4),
        ("render", JobKind::Render, 5, 9, 6),
    ] {
        assert_eq!(
            store.load_job(id).unwrap().unwrap(),
            StoredJob {
                id: id.into(),
                session_id: "session".into(),
                kind,
                input: vec![input],
                version,
                state: vec![state],
            }
        );
    }
    let job = StoredJob {
        id: "loudness".into(),
        session_id: "session".into(),
        kind: JobKind::Loudness,
        input: b"immutable hash/revision/measurement method".to_vec(),
        version: 0,
        state: b"queued".to_vec(),
    };
    store.create_job(&job).unwrap();
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Loudness), Some("session"), None, 10)
            .unwrap(),
        ["loudness"]
    );
    for kind in [JobKind::Review, JobKind::Speech, JobKind::Render] {
        assert!(matches!(
            store.create_job(&StoredJob {
                kind,
                ..job.clone()
            }),
            Err(StorageError::Conflict)
        ));
        assert!(matches!(
            store.update_job(&job.id, kind, 0, b"wrong"),
            Err(StorageError::Conflict)
        ));
    }
    let mut other = RecoveryStore::open(&db.0).unwrap();
    store
        .update_job(&job.id, JobKind::Loudness, 0, b"running")
        .unwrap();
    let observed = other.load_job(&job.id).unwrap().unwrap();
    store
        .update_job(&job.id, JobKind::Loudness, 1, b"cancelling")
        .unwrap();
    assert!(matches!(
        other.update_job(&job.id, JobKind::Loudness, observed.version, b"succeeded"),
        Err(StorageError::Conflict)
    ));
    store
        .update_job(&job.id, JobKind::Loudness, 2, b"cancelled")
        .unwrap();
    let finished = store.load_job(&job.id).unwrap().unwrap();
    assert_eq!(store.create_job(&job).unwrap(), finished);
    assert!(matches!(
        store.create_job(&StoredJob {
            input: b"different asset".to_vec(),
            ..job
        }),
        Err(StorageError::Conflict)
    ));
    drop(other);
    drop(store);
    assert_eq!(
        RecoveryStore::open(&db.0)
            .unwrap()
            .load_job("loudness")
            .unwrap()
            .unwrap(),
        finished
    );
    let raw = rusqlite::Connection::open(&db.0).unwrap();
    assert_eq!(
        raw.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        18
    );
    assert!(
        raw.execute("UPDATE jobs SET kind='unknown' WHERE id='loudness'", [])
            .is_err()
    );
}
#[test]
fn failed_upgrade_rolls_back_table_and_version_without_losing_old_jobs() {
    let db = Database::new();
    let raw = db.seed_v8();
    // Inject a conflicting staging table to force the migration to fail atomically.
    raw.execute_batch("CREATE TABLE jobs_v9(sentinel TEXT); INSERT INTO jobs_v9 VALUES('keep');")
        .unwrap();
    assert!(RecoveryStore::open(&db.0).is_err());
    assert_eq!(
        raw.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        8
    );
    assert_eq!(
        raw.query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        raw.query_row("SELECT sentinel FROM jobs_v9", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "keep"
    );
    raw.execute_batch("DROP TABLE jobs_v9;").unwrap();
    drop(raw);
    assert_eq!(
        RecoveryStore::open(&db.0)
            .unwrap()
            .list_jobs(None, None, None, 10)
            .unwrap()
            .len(),
        3
    );
}
