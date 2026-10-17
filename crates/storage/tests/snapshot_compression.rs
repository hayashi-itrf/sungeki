use rusqlite::{Connection, params};
use sungeki_storage::{RecoveryStore, StorageError, StoredSession};

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("snapshot-{}.sqlite", uuid::Uuid::new_v4())))
    }
    fn raw(&self) -> Connection {
        Connection::open(&self.0).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn state(bytes: Vec<u8>) -> StoredSession {
    StoredSession {
        id: "s".into(),
        revision: 0,
        version: 0,
        snapshot: bytes.into(),
    }
}
fn encoded(raw: &Connection) -> Vec<u8> {
    raw.query_row("SELECT snapshot FROM sessions WHERE id='s'", [], |r| {
        r.get(0)
    })
    .unwrap()
}

#[test]
fn durable_roundtrip_compresses_history_without_changing_public_bytes() {
    let fixture = Fixture::new();
    let bytes = r#"{"history":["日本語","Undo","receipt"]}"#.as_bytes().repeat(10000);
    let original = state(bytes);
    let mut store = RecoveryStore::open(&fixture.0).unwrap();
    store.commit(None, &original).unwrap();
    assert!(encoded(&fixture.raw()).len() < original.snapshot.len() / 10);
    drop(store);
    let store = RecoveryStore::open(&fixture.0).unwrap();
    assert_eq!(store.load("s").unwrap().unwrap(), original);
}

#[test]
fn shared_reads_isolate_caller_changes_and_keep_the_last_durable_write_on_failure() {
    use std::sync::Arc;
    let fixture = Fixture::new();
    let mut store = RecoveryStore::open(&fixture.0).unwrap();
    let mut original = state(vec![7; 1024 * 1024]);
    store.commit(None, &original).unwrap();
    let first = store.load("s").unwrap().unwrap();
    let mut second = store.load("s").unwrap().unwrap();
    assert!(Arc::ptr_eq(&original.snapshot, &first.snapshot));
    assert!(Arc::ptr_eq(&first.snapshot, &second.snapshot));
    // Even an owner changing its candidate cannot mutate the verified cache.
    Arc::make_mut(&mut original.snapshot)[0] = 1;
    Arc::make_mut(&mut second.snapshot)[0] = 2;
    assert_eq!(first.snapshot[0], 7);
    assert!(Arc::ptr_eq(
        &first.snapshot,
        &store.load("s").unwrap().unwrap().snapshot
    ));
    second.version = 1;
    fixture.raw().execute_batch("CREATE TRIGGER fail_shared BEFORE UPDATE ON sessions BEGIN SELECT RAISE(ABORT,'fault'); END;").unwrap();
    assert!(matches!(
        store.commit(Some(0), &second),
        Err(StorageError::Sqlite(_))
    ));
    assert_eq!(store.load("s").unwrap().unwrap(), first);
    fixture
        .raw()
        .execute_batch("DROP TRIGGER fail_shared;")
        .unwrap();
    store.commit(Some(0), &second).unwrap();
    assert!(Arc::ptr_eq(
        &second.snapshot,
        &store.load("s").unwrap().unwrap().snapshot
    ));
    drop(store);
    assert_eq!(
        RecoveryStore::open(&fixture.0)
            .unwrap()
            .load("s")
            .unwrap()
            .unwrap(),
        second
    );
}

#[test]
fn corruption_truncation_trailing_stream_and_wrong_lengths_are_rejected() {
    let fixture = Fixture::new();
    let mut store = RecoveryStore::open(&fixture.0).unwrap();
    store.commit(None, &state(vec![7; 10000])).unwrap();
    let raw = fixture.raw();
    let good = encoded(&raw);
    let mut cases = vec![vec![], good[..47].to_vec(), good[..good.len() - 1].to_vec()];
    let mut corrupt = good.clone();
    corrupt[16] ^= 1;
    cases.push(corrupt);
    let mut corrupt = good.clone();
    corrupt[0] ^= 1;
    cases.push(corrupt);
    for size in [0u64, 9999, 10001, u64::MAX] {
        let mut corrupt = good.clone();
        corrupt[8..16].copy_from_slice(&size.to_le_bytes());
        cases.push(corrupt);
    }
    let mut trailing = good.clone();
    trailing.extend_from_slice(&good[48..]);
    cases.push(trailing);
    for bad in cases {
        raw.execute("UPDATE sessions SET snapshot=?1", [bad])
            .unwrap();
        assert!(matches!(
            store.load("s"),
            Err(StorageError::InvalidSnapshot)
        ));
    }
    raw.execute("UPDATE sessions SET snapshot=?1", [good])
        .unwrap();
    assert_eq!(
        store.load("s").unwrap().unwrap().snapshot.as_slice(),
        vec![7; 10000]
    );
}

#[test]
fn v16_migration_is_atomic_and_runs_once() {
    let fixture = Fixture::new();
    drop(RecoveryStore::open(&fixture.0).unwrap());
    let raw = fixture.raw();
    for id in ["s", "t"] {
        raw.execute(
            "INSERT INTO sessions VALUES(?1,3,4,?2)",
            params![id, b"old Sungeki bytes".as_slice()],
        )
        .unwrap();
    }
    raw.execute_batch("PRAGMA user_version=16; CREATE TRIGGER fail_migration BEFORE UPDATE ON sessions WHEN OLD.id='t' BEGIN SELECT RAISE(ABORT,'fault'); END;").unwrap();
    assert!(RecoveryStore::open(&fixture.0).is_err());
    assert_eq!(encoded(&raw), b"old Sungeki bytes");
    assert_eq!(
        raw.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        16
    );
    raw.execute_batch("DROP TRIGGER fail_migration;").unwrap();
    let store = RecoveryStore::open(&fixture.0).unwrap();
    for id in ["s", "t"] {
        let value = store.load(id).unwrap().unwrap();
        assert_eq!(
            (value.revision, value.version, value.snapshot.as_slice()),
            (3, 4, b"old Sungeki bytes".as_slice())
        );
    }
    let first = encoded(&raw);
    drop(store);
    let store = RecoveryStore::open(&fixture.0).unwrap();
    assert_eq!(encoded(&raw), first);
    assert_eq!(
        store.load("s").unwrap().unwrap().snapshot.as_slice(),
        b"old Sungeki bytes"
    );
}

#[test]
fn v17_zlib_migration_verifies_old_content_and_rolls_back_corruption() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let fixture = Fixture::new();
    drop(RecoveryStore::open(&fixture.0).unwrap());
    let raw = fixture.raw();
    let original = "日本語の履歴と成功操作receipt".repeat(1000).into_bytes();
    let mut header = b"VVMREC01".to_vec();
    header.extend_from_slice(&(original.len() as u64).to_le_bytes());
    header.extend_from_slice(&Sha256::digest(&original));
    let mut encoder = flate2::write::ZlibEncoder::new(header, flate2::Compression::fast());
    encoder.write_all(&original).unwrap();
    let old = encoder.finish().unwrap();
    raw.execute("INSERT INTO sessions VALUES('s',7,9,?1)", [&old])
        .unwrap();
    raw.execute("INSERT INTO sessions VALUES('t',8,10,X'00')", [])
        .unwrap();
    raw.pragma_update(None, "user_version", 17).unwrap();
    assert!(matches!(
        RecoveryStore::open(&fixture.0),
        Err(StorageError::InvalidSnapshot)
    ));
    assert_eq!(encoded(&raw), old);
    assert_eq!(
        raw.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        17
    );
    raw.execute("UPDATE sessions SET snapshot=?1 WHERE id='t'", [&old])
        .unwrap();
    let store = RecoveryStore::open(&fixture.0).unwrap();
    assert_eq!(
        store.load("s").unwrap().unwrap().snapshot.as_slice(),
        original
    );
    assert_eq!(
        store.load("t").unwrap().unwrap().snapshot.as_slice(),
        original
    );
    assert_eq!(&encoded(&raw)[..8], b"VVMREC02");
    assert_eq!(store.load("s").unwrap().unwrap().version, 9);
}
