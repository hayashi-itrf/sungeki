use sungeki_storage::*;
#[test]
fn abrupt_job_writer() {
    let Some(path) = std::env::var_os("VVM_JOB_STORAGE_TEST_CHILD") else {
        return;
    };
    let mut store = RecoveryStore::open(std::path::Path::new(&path)).unwrap();
    store
        .commit(
            None,
            &StoredSession {
                id: "session".into(),
                revision: 0,
                version: 0,
                snapshot: b"project".to_vec().into(),
            },
        )
        .unwrap();
    store
        .create_job(&StoredJob {
            id: "job".into(),
            kind: JobKind::Review,
            session_id: "session".into(),
            input: b"fixed input".to_vec(),
            version: 0,
            state: b"queued".to_vec(),
        })
        .unwrap();
    store
        .update_job("job", JobKind::Review, 0, b"running:structure:0.5")
        .unwrap();
    std::process::exit(73); // Skip destructors and SQLite close/checkpoint.
}
#[test]
fn acknowledged_job_input_and_progress_survive_abrupt_exit() {
    let dir = std::env::temp_dir().join(format!("vvm-job-crash-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "abrupt_job_writer"])
        .env("VVM_JOB_STORAGE_TEST_CHILD", &path)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    {
        let mut store = RecoveryStore::open(&path).unwrap();
        let retained = store.load_job("job").unwrap().unwrap();
        assert_eq!(retained.input, b"fixed input");
        assert_eq!(retained.version, 1);
        assert_eq!(retained.state, b"running:structure:0.5");
        // Host recovery must explicitly commit interrupted, never automatically run again.
        store
            .update_job(
                "job",
                JobKind::Review,
                retained.version,
                b"interrupted:structure:0.5",
            )
            .unwrap();
    }
    {
        let store = RecoveryStore::open(&path).unwrap();
        assert_eq!(
            store.load_job("job").unwrap().unwrap().state,
            b"interrupted:structure:0.5"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}
