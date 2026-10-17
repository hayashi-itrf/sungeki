use rusqlite::Connection;
use std::fs;
use sungeki_storage::{JobKind, RecoveryStore, StoredJob};
#[test]
fn v12_upgrade_preserves_every_job_kind_and_accepts_collection_records() {
    let root = std::env::temp_dir().join(format!("collect-db-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("recovery.sqlite");
    let raw = Connection::open(&path).unwrap();
    raw.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY,revision INTEGER,version INTEGER,snapshot BLOB);
        INSERT INTO sessions VALUES('session',0,0,X'00');
        CREATE TABLE jobs(id TEXT PRIMARY KEY,session_id TEXT,input BLOB,version INTEGER,state BLOB,
        kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview','build_video','frame_preview')));
        PRAGMA user_version=12;").unwrap();
    for (i, name) in [
        "review",
        "speech",
        "render",
        "loudness",
        "audio_preview",
        "build_video",
        "frame_preview",
    ]
    .iter()
    .enumerate()
    {
        raw.execute(
            "INSERT INTO jobs VALUES(?1,'session',X'0123',4,X'4567',?2)",
            rusqlite::params![format!("job-{i}"), name],
        )
        .unwrap();
    }
    drop(raw);
    let mut store = RecoveryStore::open(&path).unwrap();
    for (i, kind) in [
        JobKind::Review,
        JobKind::Speech,
        JobKind::Render,
        JobKind::Loudness,
        JobKind::AudioPreview,
        JobKind::BuildVideo,
        JobKind::FramePreview,
    ]
    .iter()
    .enumerate()
    {
        let job = store.load_job(&format!("job-{i}")).unwrap().unwrap();
        assert_eq!(job.kind, *kind);
        assert_eq!(job.version, 4);
        assert_eq!(job.input, [1, 35]);
        assert_eq!(job.state, [69, 103]);
    }
    let request = StoredJob {
        id: "collection-job".into(),
        session_id: "session".into(),
        kind: JobKind::Collection,
        version: 0,
        input: b"immutable input".to_vec(),
        state: b"queued".to_vec(),
    };
    store.create_job(&request).unwrap();
    store
        .update_job(&request.id, JobKind::Collection, 0, b"intent")
        .unwrap();
    drop(store);
    let mut store = RecoveryStore::open(&path).unwrap();
    let replay = store.create_job(&request).unwrap();
    assert_eq!(replay.version, 1);
    assert_eq!(replay.state, b"intent");
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Collection), None, None, 10)
            .unwrap(),
        ["collection-job"]
    );
    drop(store);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn v13_collection_messages_become_typed_without_changing_identity_or_version() {
    let root = std::env::temp_dir().join(format!("collect-error-db-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("db");
    drop(RecoveryStore::open(&path).unwrap());
    let raw = Connection::open(&path).unwrap();
    raw.execute("INSERT INTO sessions VALUES('s',0,0,X'00')", [])
        .unwrap();
    for phase in ["failed", "cancelled", "interrupted"] {
        let state = serde_json::json!({"phase":phase,"receipt":null,"error":"保存された原因"});
        raw.execute(
            "INSERT INTO jobs VALUES(?1,'s',X'01',7,?2,'collection')",
            rusqlite::params![phase, serde_json::to_vec(&state).unwrap()],
        )
        .unwrap();
    }
    raw.pragma_update(None, "user_version", 13).unwrap();
    drop(raw);
    let store = RecoveryStore::open(&path).unwrap();
    for phase in ["failed", "cancelled", "interrupted"] {
        let job = store.load_job(phase).unwrap().unwrap();
        let state: serde_json::Value = serde_json::from_slice(&job.state).unwrap();
        assert_eq!(job.version, 7);
        assert_eq!(job.input, [1]);
        assert_eq!(state["phase"], phase);
        assert_eq!(state["error"]["message"], "保存された原因");
        assert_eq!(
            state["error"]["code"],
            if phase == "cancelled" {
                "CANCELLED"
            } else {
                "IO_ERROR"
            }
        );
        assert_eq!(state["error"]["retryable"], false);
    }
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
