use super::*;
use std::os::unix::fs::symlink;

#[test]
fn known_conflict_is_rejected_before_preflight_without_touching_existing_settings() {
    for engine in ["sbv2", "qwen", "irodori"] {
        let directory = crate::speech_journal_tests::Fixture::new();
        let source = directory.0.join("candidate.json");
        let destination = directory.0.join(format!("{engine}.json"));
        // Invalid JSON distinguishes early conflict detection from preflight failure.
        fs::write(&source, b"uninspected candidate").unwrap();
        fs::write(&destination, b"existing settings").unwrap();
        let error = install(&directory.0, engine, &source, &AtomicBool::new(false)).unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert_eq!(fs::read(&destination).unwrap(), b"existing settings");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }
}

#[test]
fn identical_retry_still_requires_valid_configuration() {
    for engine in ["sbv2", "qwen", "irodori"] {
        let directory = crate::speech_journal_tests::Fixture::new();
        let source = directory.0.join("candidate.json");
        let destination = directory.0.join(format!("{engine}.json"));
        let bytes = b"{\"format_version\":1,\"profiles\":[]}";
        fs::write(&source, bytes).unwrap();
        fs::write(&destination, bytes).unwrap();
        let error = install(&directory.0, engine, &source, &AtomicBool::new(false)).unwrap_err();
        assert_eq!(error.code, ErrorCode::ValidationFailed);
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }
}

#[test]
fn publication_is_atomic_no_clobber_and_identical_retry_is_safe() {
    let directory = crate::speech_journal_tests::Fixture::new();
    let temporary = directory.0.as_path().join("candidate");
    let destination = directory.0.as_path().join("qwen.json");
    fs::write(&temporary, b"validated configuration").unwrap();
    let cancel = AtomicBool::new(false);
    publish(
        &temporary,
        &destination,
        b"validated configuration",
        &cancel,
    )
    .unwrap();
    publish(
        &temporary,
        &destination,
        b"validated configuration",
        &cancel,
    )
    .unwrap();
    let other = directory.0.as_path().join("other");
    fs::write(&other, b"different configuration").unwrap();
    assert!(publish(&other, &destination, b"different configuration", &cancel).is_err());
    assert_eq!(fs::read(destination).unwrap(), b"validated configuration");
}

#[test]
fn cancelled_publication_and_symlink_destination_do_not_modify_settings() {
    let directory = crate::speech_journal_tests::Fixture::new();
    let temporary = directory.0.as_path().join("candidate");
    let destination = directory.0.as_path().join("sbv2.json");
    fs::write(&temporary, b"bytes").unwrap();
    assert!(publish(&temporary, &destination, b"bytes", &AtomicBool::new(true)).is_err());
    assert!(!destination.exists());
    symlink(&temporary, &destination).unwrap();
    assert!(publish(&temporary, &destination, b"bytes", &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(&temporary).unwrap(), b"bytes");
    assert!(
        fs::symlink_metadata(destination)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn invalid_candidates_never_install_and_leave_no_temporary_files() {
    for engine in ["sbv2", "qwen", "irodori"] {
        let directory = crate::speech_journal_tests::Fixture::new();
        let source = directory.0.as_path().join("candidate.json");
        fs::write(&source, b"{\"format_version\":1,\"profiles\":[]}").unwrap();
        assert!(
            install(
                directory.0.as_path(),
                engine,
                &source,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            !directory
                .0
                .as_path()
                .join(format!("{engine}.json"))
                .exists()
        );
        assert_eq!(fs::read_dir(directory.0.as_path()).unwrap().count(), 1);
    }
}

#[test]
fn simultaneous_different_registrations_publish_exactly_one_complete_file() {
    let directory = crate::speech_journal_tests::Fixture::new();
    let destination = directory.0.join("qwen.json");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let attempts: Vec<_> = [
        b"first configuration".as_slice(),
        b"second configuration".as_slice(),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, bytes)| {
        let temporary = directory.0.join(format!("candidate-{index}"));
        fs::write(&temporary, bytes).unwrap();
        let destination = destination.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            publish(&temporary, &destination, bytes, &AtomicBool::new(false))
        })
    })
    .collect();
    let results: Vec<_> = attempts
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| result
                .as_ref()
                .is_err_and(|error| error.code == ErrorCode::Conflict))
            .count(),
        1
    );
    let saved = fs::read(destination).unwrap();
    assert!(saved == b"first configuration" || saved == b"second configuration");
}
