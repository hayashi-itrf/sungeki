//! Measure durable writes of a representative opaque session snapshot.
use std::{path::Path, time::Instant};
use sungeki_storage::{RecoveryStore, StoredSession};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let snapshot = std::fs::read(&args[1]).unwrap();
    assert!(!Path::new(&args[2]).exists());
    let mut store = RecoveryStore::open(Path::new(&args[2])).unwrap();
    let mut row = StoredSession {
        id: "profile".into(),
        revision: 0,
        version: 0,
        snapshot: snapshot.into(),
    };
    store.commit(None, &row).unwrap();
    for version in 1..=3 {
        let began = Instant::now();
        let previous = store.load("profile").unwrap().unwrap();
        let load = began.elapsed();
        row.version = version;
        let began = Instant::now();
        store.commit(Some(previous.version), &row).unwrap();
        println!(
            "{}",
            serde_json::json!({"load_ms":load.as_secs_f64()*1000.,
            "commit_ms":began.elapsed().as_secs_f64()*1000.,"bytes":row.snapshot.len()})
        );
    }
}
