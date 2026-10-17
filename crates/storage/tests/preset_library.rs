use std::{
    path::PathBuf,
    sync::{Arc, Barrier},
};
use sungeki_storage::{PresetLibrary, StorageError};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("vvm-presets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("presets.sqlite")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn restart_retry_delete_and_stale_creation_preserve_history() {
    let fixture = Fixture::new();
    let mut library = PresetLibrary::open(&fixture.path()).unwrap();
    let original = library.write("voice-a", 0, Some(b"first")).unwrap();
    drop(library);
    let mut library = PresetLibrary::open(&fixture.path()).unwrap();
    assert_eq!(library.get("voice-a").unwrap(), Some(original.clone()));
    assert_eq!(
        library.write("voice-a", 0, Some(b"first")).unwrap(),
        original
    );
    assert!(matches!(
        library.write("voice-a", 0, Some(b"other")),
        Err(StorageError::Conflict)
    ));
    let updated = library.write("voice-a", 1, Some(b"second")).unwrap();
    assert_eq!(updated.revision, 2);
    let deleted = library.write("voice-a", 2, None).unwrap();
    drop(library);
    let mut library = PresetLibrary::open(&fixture.path()).unwrap();
    assert_eq!(library.write("voice-a", 2, None).unwrap(), deleted);
    assert_eq!(library.get("voice-a").unwrap(), Some(deleted));
    assert!(library.list("", 100).unwrap().is_empty());
    for revision in [0, 1, 2, 3] {
        assert!(matches!(
            library.write("voice-a", revision, Some(b"first")),
            Err(StorageError::Conflict)
        ));
    }
    assert!(matches!(
        library.write("unknown", 0, None),
        Err(StorageError::Conflict)
    ));
    assert!(library.get("unknown").unwrap().is_none());
}

#[test]
fn simultaneous_writers_have_one_winner_and_no_mixed_payload() {
    let fixture = Fixture::new();
    let mut library = PresetLibrary::open(&fixture.path()).unwrap();
    library.write("shared", 0, Some(b"initial")).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [b"first".to_vec(), b"second".to_vec()]
        .into_iter()
        .map(|payload| {
            let path = fixture.path();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut library = PresetLibrary::open(&path).unwrap();
                barrier.wait();
                library.write("shared", 1, Some(&payload))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StorageError::Conflict)))
            .count(),
        1
    );
    assert_eq!(
        library.get("shared").unwrap().as_ref(),
        results.iter().find_map(|r| r.as_ref().ok())
    );
}

#[test]
fn pagination_and_invalid_requests_leave_records_unchanged() {
    let fixture = Fixture::new();
    let mut library = PresetLibrary::open(&fixture.path()).unwrap();
    for id in ["c", "a", "b"] {
        library.write(id, 0, Some(b"valid")).unwrap();
    }
    assert_eq!(
        library
            .list("", 2)
            .unwrap()
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(library.list("b", 2).unwrap()[0].id, "c");
    assert!(library.list("", 0).unwrap().is_empty());
    let before = library.get("a").unwrap();
    assert!(matches!(
        library.write("a", 1, Some(b"")),
        Err(StorageError::InvalidSnapshot)
    ));
    assert!(matches!(
        library.write("a", u32::MAX, Some(b"x")),
        Err(StorageError::InvalidVersion)
    ));
    assert_eq!(library.get("a").unwrap(), before);
}

#[test]
fn failed_database_write_preserves_committed_record_after_reopen() {
    let fixture = Fixture::new();
    let mut library = PresetLibrary::open(&fixture.path()).unwrap();
    let original = library.write("voice", 0, Some(b"original")).unwrap();
    let connection = rusqlite::Connection::open(fixture.path()).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_update BEFORE UPDATE ON presets
         BEGIN SELECT RAISE(ABORT, 'injected storage failure'); END;",
        )
        .unwrap();
    assert!(matches!(
        library.write("voice", 1, Some(b"replacement")),
        Err(StorageError::Sqlite(_))
    ));
    assert!(matches!(
        library.write("voice", 1, None),
        Err(StorageError::Sqlite(_))
    ));
    drop(library);
    drop(connection);
    let library = PresetLibrary::open(&fixture.path()).unwrap();
    assert_eq!(library.get("voice").unwrap(), Some(original));
}
