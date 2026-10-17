//! Developer probe, not GUI/MCP acceptance: isolate editor and recovery costs.
//! cargo run --release --locked -p sungeki-host --example profile_edit_history -- PROJECT NEW_DB
use std::{path::Path, time::Instant};
use sungeki_editor::{Command, EditAction, EditRequest, EditSession};
use sungeki_storage::{RecoveryStore, StoredSession, load_document};

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3, "expected PROJECT NEW_DB");
    let destination = Path::new(&args[2]);
    // Reserve exclusively; never open or alter another session's database.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    let project = load_document(Path::new(&args[1])).unwrap().project;
    let clip_id = project.clips[0].id.clone();
    let start_us = project.clips[0].start_us;
    let mut session = EditSession::new(project).unwrap();
    let mut store = RecoveryStore::open(destination).unwrap();
    let mut stored = StoredSession {
        id: uuid::Uuid::new_v4().to_string(),
        revision: 0,
        version: 0,
        snapshot: session.snapshot().unwrap().into(),
    };
    store.commit(None, &stored).unwrap();
    let mut rows = Vec::new();
    for n in 1..=120 {
        let begin = Instant::now();
        let loaded = store.load(&stored.id).unwrap().unwrap();
        assert_eq!(loaded, stored);
        let load_ms = ms(begin);
        let begin = Instant::now();
        let mut candidate = session.clone();
        candidate
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: session.history().revision(),
                action: EditAction::Apply {
                    commands: vec![Command::MoveClip {
                        clip_id: clip_id.clone(),
                        start_us: sungeki_project::TimeUs(
                            start_us.0 + if n % 2 == 1 { 1000 } else { 0 },
                        ),
                    }],
                },
            })
            .unwrap();
        let edit_ms = ms(begin);
        let begin = Instant::now();
        let snapshot = candidate.snapshot().unwrap();
        let serialize_ms = ms(begin);
        let next = StoredSession {
            id: stored.id.clone(),
            revision: candidate.history().revision(),
            version: stored.version + 1,
            snapshot: snapshot.into(),
        };
        let begin = Instant::now();
        store.commit(Some(stored.version), &next).unwrap();
        let commit_ms = ms(begin);
        rows.push(
            serde_json::json!({ "edit": n, "undo_entries": candidate.history().undo_len(),
            "snapshot_bytes": next.snapshot.len(), "load_and_compare_ms": load_ms,
            "clone_and_edit_ms": edit_ms, "serialize_ms": serialize_ms, "commit_ms": commit_ms }),
        );
        session = candidate;
        stored = next;
    }
    drop(store);
    let reopened = RecoveryStore::open(destination)
        .unwrap()
        .load(&stored.id)
        .unwrap()
        .unwrap();
    assert_eq!(reopened, stored);
    let restored = EditSession::restore(&reopened.snapshot).unwrap();
    assert_eq!(restored.history().project(), session.history().project());
    assert_eq!(restored.history().undo_len(), session.history().undo_len());
    println!(
        "{}",
        serde_json::json!({ "scope": "editor/storage only; no host protocol, GUI, or IPC",
        "source": args[1], "database": args[2], "reopen_verified": true, "rows": rows })
    );
}
