//! Fixed-input diagnostic. Uses the production codec and never modifies the source DB.
//! cargo run --release --locked -p sungeki-storage --example profile_storage_stages -- SOURCE_DB NEW_DB
#[allow(dead_code)]
#[path = "../src/snapshot_codec.rs"]
mod snapshot_codec;
use rusqlite::{Connection, OpenFlags, params};
use std::{path::Path, time::Instant};
use sungeki_storage::{RecoveryStore, StorageError, StoredSession};
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "expected SOURCE_DB NEW_DB");
    let source = Connection::open_with_flags(&args[1], OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let encoded: Vec<u8> = source
        .query_row("SELECT snapshot FROM sessions LIMIT 1", [], |r| r.get(0))
        .unwrap();
    let raw = snapshot_codec::decode(&encoded).unwrap();
    drop(source);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])
        .unwrap();
    let mut store = RecoveryStore::open(Path::new(&args[2])).unwrap();
    let mut row = StoredSession {
        id: "probe".into(),
        revision: 0,
        version: 0,
        snapshot: raw.into(),
    };
    store.commit(None, &row).unwrap();
    let mut db = Connection::open(&args[2]).unwrap();
    db.pragma_update(None, "synchronous", "FULL").unwrap();
    db.execute(
        "CREATE TABLE stage_probe (id INTEGER PRIMARY KEY, snapshot BLOB)",
        [],
    )
    .unwrap();
    db.execute("INSERT INTO stage_probe VALUES(1,?1)", [&encoded])
        .unwrap();
    for n in 0..10 {
        let t = Instant::now();
        let compressed = snapshot_codec::encode(&row.snapshot).unwrap();
        let encode_ms = ms(t);
        let t = Instant::now();
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        tx.execute(
            "UPDATE stage_probe SET snapshot=?1 WHERE id=1",
            params![compressed],
        )
        .unwrap();
        tx.commit().unwrap();
        let write_ms = ms(t);
        let t = Instant::now();
        let shared_reference = row.snapshot.clone();
        std::hint::black_box(&shared_reference);
        let shared_reference_ms = ms(t);
        drop(shared_reference);
        let t = Instant::now();
        let copy_control = row.snapshot.as_ref().clone();
        std::hint::black_box(&copy_control);
        let copy_control_ms = ms(t);
        assert_eq!(copy_control.as_slice(), row.snapshot.as_slice());
        drop(copy_control);
        let t = Instant::now();
        let loaded = store.load("probe").unwrap().unwrap();
        let warm_load_ms = ms(t);
        assert!(std::sync::Arc::ptr_eq(&row.snapshot, &loaded.snapshot));
        row.version += 1;
        let t = Instant::now();
        store.commit(Some(row.version - 1), &row).unwrap();
        let full_commit_ms = ms(t);
        println!(
            "{}",
            serde_json::json!({"iteration":n,"raw_bytes":row.snapshot.len(),
            "encoded_bytes":compressed.len(),"encode_hash_ms":encode_ms,"sqlite_full_write_ms":write_ms,
            "shared_reference_ms":shared_reference_ms,"copy_control_ms":copy_control_ms,
            "warm_load_ms":warm_load_ms,"production_commit_ms":full_commit_ms})
        );
    }
    drop(db);
    drop(store);
    let loaded = RecoveryStore::open(Path::new(&args[2]))
        .unwrap()
        .load("probe")
        .unwrap()
        .unwrap();
    assert_eq!(loaded, row);
    eprintln!("durable reload matches input and generation");
}
