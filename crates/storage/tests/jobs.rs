use std::{fs, path::PathBuf};
use sungeki_storage::*;
struct Fixture {
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("vvm-jobs-{}.sqlite", uuid::Uuid::new_v4()));
        Self { path }
    }
    fn store(&self) -> RecoveryStore {
        RecoveryStore::open(&self.path).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}
fn seed(store: &mut RecoveryStore, id: &str) {
    store
        .commit(
            None,
            &StoredSession {
                id: id.into(),
                revision: 0,
                version: 0,
                snapshot: b"session".to_vec().into(),
            },
        )
        .unwrap();
}
fn job(id: &str, session: &str) -> StoredJob {
    StoredJob {
        kind: JobKind::Review,
        id: id.into(),
        session_id: session.into(),
        input: b"immutable project revision/hash/options".to_vec(),
        version: 0,
        state: b"queued".to_vec(),
    }
}
#[test]
fn repeated_start_retains_latest_state_and_immutable_input_after_reopen() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    seed(&mut store, "session");
    let request = job("job", "session");
    store.create_job(&request).unwrap();
    store
        .update_job("job", JobKind::Review, 0, b"running")
        .unwrap();
    let finished = b"succeeded with result references";
    store
        .update_job("job", JobKind::Review, 1, finished)
        .unwrap();
    drop(store);
    let mut store = fixture.store();
    let replay = store.create_job(&request).unwrap();
    assert_eq!(replay.version, 2);
    assert_eq!(replay.state, finished);
    assert_eq!(replay.input, request.input);
    assert!(matches!(
        store.create_job(&StoredJob {
            input: b"different snapshot".to_vec(),
            ..request.clone()
        }),
        Err(StorageError::Conflict)
    ));
    assert!(matches!(
        store.create_job(&StoredJob {
            session_id: "other".into(),
            ..request
        }),
        Err(StorageError::Conflict)
    ));
    assert_eq!(store.load_job("job").unwrap().unwrap(), replay);
}
#[test]
fn concurrent_completion_cannot_overwrite_cancel_state() {
    let fixture = Fixture::new();
    let mut first = fixture.store();
    seed(&mut first, "session");
    first.create_job(&job("job", "session")).unwrap();
    let mut second = fixture.store();
    first
        .update_job("job", JobKind::Review, 0, b"running")
        .unwrap();
    let observed = second.load_job("job").unwrap().unwrap();
    first
        .update_job("job", JobKind::Review, 1, b"cancelling")
        .unwrap();
    assert!(matches!(
        second.update_job("job", JobKind::Review, observed.version, b"succeeded"),
        Err(StorageError::Conflict)
    ));
    assert_eq!(first.load_job("job").unwrap().unwrap().state, b"cancelling");
    second
        .update_job("job", JobKind::Review, 2, b"cancelled after cleanup")
        .unwrap();
    assert_eq!(first.load_job("job").unwrap().unwrap().version, 3);
}
#[test]
fn failed_write_does_not_publish_progress_or_results() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    seed(&mut store, "session");
    let initial = store.create_job(&job("job", "session")).unwrap();
    let raw = rusqlite::Connection::open(&fixture.path).unwrap();
    raw.execute_batch("CREATE TRIGGER fail_job BEFORE UPDATE ON jobs BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert!(matches!(
        store.update_job("job", JobKind::Review, 0, b"succeeded"),
        Err(StorageError::Sqlite(_))
    ));
    assert_eq!(store.load_job("job").unwrap().unwrap(), initial);
}
#[test]
fn bounded_listing_separates_sessions_and_rejects_invalid_creates() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    assert!(store.create_job(&job("missing", "absent")).is_err());
    seed(&mut store, "a");
    seed(&mut store, "b");
    for (id, session) in [("01", "a"), ("02", "b"), ("03", "a")] {
        store.create_job(&job(id, session)).unwrap();
    }
    assert_eq!(store.list_jobs(None, None, None, 2).unwrap(), ["01", "02"]);
    assert_eq!(store.list_jobs(None, None, Some("02"), 2).unwrap(), ["03"]);
    assert_eq!(
        store.list_jobs(None, Some("a"), None, 10).unwrap(),
        ["01", "03"]
    );
    assert!(store.list_jobs(None, None, None, 101).is_err());
    assert!(store.list_jobs(None, None, None, 0).is_err());
    assert!(
        store
            .create_job(&StoredJob {
                version: 1,
                ..job("new", "a")
            })
            .is_err()
    );
    assert!(matches!(
        store.update_job("01", JobKind::Review, u32::MAX, b"x"),
        Err(StorageError::InvalidVersion)
    ));
    assert_eq!(store.load_job("unknown").unwrap(), None);
}

#[test]
fn adding_job_table_preserves_existing_sessions_and_rejects_future_schema() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    seed(&mut store, "existing");
    drop(store);
    let raw = rusqlite::Connection::open(&fixture.path).unwrap();
    raw.execute_batch("DROP TABLE jobs; UPDATE sessions SET snapshot=CAST('session' AS BLOB); PRAGMA user_version=5;")
        .unwrap();
    drop(raw);
    let mut store = fixture.store();
    assert_eq!(
        store.load("existing").unwrap().unwrap().snapshot.as_slice(),
        b"session"
    );
    store.create_job(&job("new", "existing")).unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(&fixture.path).unwrap();
    let version: u32 = raw
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 18);
    raw.pragma_update(None, "user_version", 19).unwrap();
    assert!(matches!(
        RecoveryStore::open(&fixture.path),
        Err(StorageError::UnsupportedVersion)
    ));
}

#[test]
fn immutable_kind_prevents_cross_kind_replay_updates_and_listing() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    seed(&mut store, "session");
    let review = job("01", "session");
    store.create_job(&review).unwrap();
    let speech = StoredJob {
        kind: JobKind::Speech,
        ..job("02", "session")
    };
    store.create_job(&speech).unwrap();
    assert!(matches!(
        store.create_job(&StoredJob {
            kind: JobKind::Speech,
            ..review.clone()
        }),
        Err(StorageError::Conflict)
    ));
    assert!(matches!(
        store.update_job("02", JobKind::Review, 0, b"incorrect"),
        Err(StorageError::Conflict)
    ));
    assert_eq!(store.load_job("02").unwrap().unwrap(), speech);
    store
        .update_job("02", JobKind::Speech, 0, b"running")
        .unwrap();
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Review), None, None, 10)
            .unwrap(),
        ["01"]
    );
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Speech), Some("session"), None, 1)
            .unwrap(),
        ["02"]
    );
    assert!(
        store
            .list_jobs(Some(JobKind::Speech), None, Some("02"), 1)
            .unwrap()
            .is_empty()
    );
    drop(store);
    let store = fixture.store();
    assert_eq!(store.load_job("02").unwrap().unwrap().kind, JobKind::Speech);
    assert_eq!(store.load_job("01").unwrap().unwrap(), review);
}

#[test]
fn next_recovery_v6_is_upgraded_atomically_without_rewriting_payloads() {
    let fixture = Fixture::new();
    let raw = rusqlite::Connection::open(&fixture.path).unwrap();
    raw.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY, revision INTEGER NOT NULL, version INTEGER NOT NULL, snapshot BLOB NOT NULL);
        INSERT INTO sessions VALUES('session',0,0,X'00');
        CREATE TABLE jobs(id TEXT PRIMARY KEY,session_id TEXT NOT NULL,input BLOB NOT NULL,version INTEGER NOT NULL,state BLOB NOT NULL);
        INSERT INTO jobs VALUES('job','session',X'0001',9,X'0203'); PRAGMA user_version=6;").unwrap();
    drop(raw);
    let store = fixture.store();
    let restored = store.load_job("job").unwrap().unwrap();
    assert_eq!(restored.kind, JobKind::Review);
    assert_eq!(restored.input, [0, 1]);
    assert_eq!(restored.state, [2, 3]);
    assert_eq!(restored.version, 9);
    drop(store);
    assert_eq!(fixture.store().load_job("job").unwrap().unwrap(), restored);
}

#[test]
fn v7_migration_preserves_speech_and_adds_isolated_render_jobs() {
    let fixture = Fixture::new();
    let raw = rusqlite::Connection::open(&fixture.path).unwrap();
    raw.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,version INTEGER NOT NULL,snapshot BLOB NOT NULL);
        INSERT INTO sessions VALUES('session',0,0,X'00');
        CREATE TABLE jobs(id TEXT PRIMARY KEY,session_id TEXT NOT NULL,input BLOB NOT NULL,version INTEGER NOT NULL,state BLOB NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('review','speech')));
        INSERT INTO jobs VALUES('speech','session',X'0001',11,X'0203','speech');
        CREATE INDEX jobs_session ON jobs(session_id,id);
        CREATE INDEX jobs_kind_session ON jobs(kind,session_id,id); PRAGMA user_version=7;").unwrap();
    drop(raw);
    let mut store = fixture.store();
    let speech = store.load_job("speech").unwrap().unwrap();
    assert_eq!(speech.kind, JobKind::Speech);
    assert_eq!(
        (speech.input, speech.version, speech.state),
        (vec![0, 1], 11, vec![2, 3])
    );
    let render = StoredJob {
        kind: JobKind::Render,
        ..job("render", "session")
    };
    store.create_job(&render).unwrap();
    assert!(matches!(
        store.update_job("render", JobKind::Speech, 0, b"wrong"),
        Err(StorageError::Conflict)
    ));
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Render), None, None, 10)
            .unwrap(),
        ["render"]
    );
    drop(store);
    assert_eq!(fixture.store().load_job("render").unwrap().unwrap(), render);
}
