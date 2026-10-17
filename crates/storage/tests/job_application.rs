use sungeki_storage::*;
struct Fixture {
    path: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        Self {
            path: std::env::temp_dir().join(format!("vvm-apply-{}.sqlite", uuid::Uuid::new_v4())),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}
fn session(id: &str, version: u32) -> StoredSession {
    StoredSession {
        id: id.into(),
        revision: version,
        version,
        snapshot: format!("project/history/receipt {version}")
            .into_bytes()
            .into(),
    }
}
fn seed(store: &mut RecoveryStore) -> StoredJob {
    store.commit(None, &session("session", 0)).unwrap();
    let job = StoredJob {
        id: "job".into(),
        session_id: "session".into(),
        kind: JobKind::Speech,
        version: 0,
        input: b"fixed input".to_vec(),
        state: b"pending".to_vec(),
    };
    store.create_job(&job).unwrap()
}
#[test]
fn both_records_survive_reopen_and_replay_does_not_apply_twice() {
    let fixture = Fixture::new();
    let mut store = RecoveryStore::open(&fixture.path).unwrap();
    let job = seed(&mut store);
    assert_eq!(
        store
            .commit_job_application(0, &session("session", 1), &job, b"applied 1")
            .unwrap(),
        1
    );
    drop(store);
    let mut store = RecoveryStore::open(&fixture.path).unwrap();
    assert_eq!(
        store.load("session").unwrap().unwrap(),
        session("session", 1)
    );
    assert_eq!(store.load_job("job").unwrap().unwrap().state, b"applied 1");
    assert!(matches!(
        store.commit_job_application(0, &session("session", 1), &job, b"applied 1"),
        Err(StorageError::Conflict)
    ));
    assert_eq!(store.load_job("job").unwrap().unwrap().version, 1);
}
#[test]
fn stale_job_wrong_owner_kind_or_input_rolls_back_session() {
    for case in 0..5 {
        let fixture = Fixture::new();
        let mut store = RecoveryStore::open(&fixture.path).unwrap();
        let original = seed(&mut store);
        let mut wrong = original.clone();
        match case {
            0 => wrong.version = 1,
            1 => wrong.session_id = "other".into(),
            2 => wrong.kind = JobKind::Review,
            3 => wrong.input = b"different".to_vec(),
            _ => wrong.id = "missing".into(),
        }
        assert!(matches!(
            store.commit_job_application(0, &session("session", 1), &wrong, b"applied"),
            Err(StorageError::Conflict)
        ));
        assert_eq!(
            store.load("session").unwrap().unwrap(),
            session("session", 0)
        );
        assert_eq!(store.load_job("job").unwrap().unwrap(), original);
    }
}
#[test]
fn fault_in_either_write_rolls_back_both_then_retry_succeeds() {
    for table in ["sessions", "jobs"] {
        let fixture = Fixture::new();
        let mut store = RecoveryStore::open(&fixture.path).unwrap();
        let job = seed(&mut store);
        let fault = rusqlite::Connection::open(&fixture.path).unwrap();
        fault.execute_batch(&format!("CREATE TRIGGER fail_apply BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        assert!(matches!(
            store.commit_job_application(0, &session("session", 1), &job, b"applied"),
            Err(StorageError::Sqlite(_))
        ));
        assert_eq!(
            store.load("session").unwrap().unwrap(),
            session("session", 0)
        );
        assert_eq!(store.load_job("job").unwrap().unwrap(), job);
        fault.execute_batch("DROP TRIGGER fail_apply").unwrap();
        store
            .commit_job_application(0, &session("session", 1), &job, b"applied")
            .unwrap();
    }
}
