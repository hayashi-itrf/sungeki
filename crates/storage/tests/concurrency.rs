use sungeki_storage::*;
#[test]
fn unchanged_project_revision_still_serializes_receipt_writes() {
    let dir = std::env::temp_dir().join(format!("vvm-cas-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db");
    {
        let mut first = RecoveryStore::open(&path).unwrap();
        let mut second = RecoveryStore::open(&path).unwrap();
        first
            .commit(
                None,
                &StoredSession {
                    id: "session".into(),
                    revision: 0,
                    version: 0,
                    snapshot: b"initial".to_vec().into(),
                },
            )
            .unwrap();
        let a = StoredSession {
            id: "session".into(),
            revision: 0,
            version: 1,
            snapshot: b"receipt-a".to_vec().into(),
        };
        first.commit(Some(0), &a).unwrap();
        let b = StoredSession {
            snapshot: b"receipt-b".to_vec().into(),
            ..a
        };
        assert!(matches!(
            second.commit(Some(0), &b),
            Err(StorageError::Conflict)
        ));
        assert_eq!(
            second.load("session").unwrap().unwrap().snapshot.as_slice(),
            b"receipt-a"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_failure_rolls_back_snapshot() {
    let dir = std::env::temp_dir().join(format!("vvm-rollback-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db");
    {
        let mut store = RecoveryStore::open(&path).unwrap();
        store
            .commit(
                None,
                &StoredSession {
                    id: "session".into(),
                    revision: 0,
                    version: 0,
                    snapshot: b"before".to_vec().into(),
                },
            )
            .unwrap();
        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("CREATE TRIGGER fail_update AFTER UPDATE ON sessions BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
        assert!(matches!(
            store.commit(
                Some(0),
                &StoredSession {
                    id: "session".into(),
                    revision: 1,
                    version: 1,
                    snapshot: b"after".to_vec().into()
                }
            ),
            Err(StorageError::Sqlite(_))
        ));
        let retained = store.load("session").unwrap().unwrap();
        assert_eq!(retained.version, 0);
        assert_eq!(retained.snapshot.as_slice(), b"before");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn abrupt_writer() {
    let Some(path) = std::env::var_os("VVM_STORAGE_TEST_CHILD") else {
        return;
    };
    let mut store = RecoveryStore::open(std::path::Path::new(&path)).unwrap();
    store
        .commit(
            None,
            &StoredSession {
                id: "crash".into(),
                revision: 0,
                version: 0,
                snapshot: b"durable".to_vec().into(),
            },
        )
        .unwrap();
    std::process::exit(73); // Deliberately skip Rust/SQLite destructors.
}

#[test]
fn committed_wal_survives_abrupt_process_exit() {
    let dir = std::env::temp_dir().join(format!("vvm-exit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "abrupt_writer"])
        .env("VVM_STORAGE_TEST_CHILD", &path)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    {
        let store = RecoveryStore::open(&path).unwrap();
        assert_eq!(
            store.load("crash").unwrap().unwrap().snapshot.as_slice(),
            b"durable"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn another_writer_invalidates_decoding_reuse_and_metadata_is_always_current() {
    let dir = std::env::temp_dir().join(format!("vvm-cache-writers-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db");
    {
        let mut first = RecoveryStore::open(&path).unwrap();
        let mut second = RecoveryStore::open(&path).unwrap();
        first
            .commit(
                None,
                &StoredSession {
                    id: "shared".into(),
                    revision: 0,
                    version: 0,
                    snapshot: b"before".to_vec().into(),
                },
            )
            .unwrap();
        assert_eq!(
            first.load("shared").unwrap().unwrap().snapshot.as_slice(),
            b"before"
        );
        let changed = StoredSession {
            id: "shared".into(),
            revision: 1,
            version: 1,
            snapshot: b"after".to_vec().into(),
        };
        second.commit(Some(0), &changed).unwrap();
        assert_eq!(first.load("shared").unwrap().unwrap(), changed);
        // Same bytes with a different generation must still expose current row metadata.
        let changed = StoredSession {
            version: 2,
            ..changed
        };
        second.commit(Some(1), &changed).unwrap();
        assert_eq!(first.load("shared").unwrap().unwrap(), changed);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
