//! Filesystem failures after planning, with no real disk exhaustion.
use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn cancellation_after_partial_asset_write_preserves_sources_and_existing_output() {
    let f = Fixture::new();
    let before = f.project.clone();
    let existing = f.root.join("completed");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("keep"), b"existing completed output").unwrap();
    let destination = f.root.join("cancelled-package");
    let copied_bytes = Cell::new(0);
    let result = execute_recorded(
        &f.project,
        &destination,
        CollectionLimits::default(),
        &uuid::Uuid::new_v4().to_string(),
        || {
            // Observe real writes rather than count callback invocations. Planning
            // and pre-copy checks must pass before cancellation becomes true.
            for stage in f.stages() {
                let Ok(entries) = fs::read_dir(stage.join("assets")) else {
                    continue;
                };
                for entry in entries {
                    let length = entry.unwrap().metadata().unwrap().len();
                    if length > 0 && length < f.bytes.len() as u64 {
                        copied_bytes.set(length);
                        return true;
                    }
                }
            }
            false
        },
        |_| panic!("partial copy cancellation must not reach publication"),
    );
    assert!(matches!(
        result,
        Err(Error::Publish(PublishError::Cancelled))
    ));
    assert!(copied_bytes.get() > 0 && copied_bytes.get() < f.bytes.len() as u64);
    assert!(!destination.exists());
    assert!(f.stages().is_empty());
    assert_eq!(f.project, before);
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert_eq!(
        fs::read(existing.join("keep")).unwrap(),
        b"existing completed output"
    );
}

#[test]
fn stage_write_denied_preserves_original_and_removes_unfinished_directory() {
    let f = Fixture::new();
    let before = f.project.clone();
    let existing = f.root.join("completed");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("keep"), b"existing completed output").unwrap();
    let destination = f.root.join("new-package");
    let result = execute_journaled(
        &f.project,
        &[],
        &destination,
        CollectionLimits::default(),
        &uuid::Uuid::new_v4().to_string(),
        || false,
        |stage| {
            // The stage exists and its recording callback runs; the first child write now
            // fails at the OS boundary, before any original is copied.
            fs::set_permissions(stage.path(), fs::Permissions::from_mode(0o500))?;
            Ok(())
        },
        |_| panic!("write failure must not reach publication"),
    );
    assert!(
        matches!(result, Err(Error::Io(ref e)) if e.kind() == std::io::ErrorKind::PermissionDenied)
    );
    assert!(!destination.exists());
    assert!(f.stages().is_empty());
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert_eq!(f.project, before);
    assert_eq!(
        fs::read(existing.join("keep")).unwrap(),
        b"existing completed output"
    );
}

#[test]
fn destination_created_after_copy_is_never_overwritten_or_reported_complete() {
    let f = Fixture::new();
    let destination = f.root.join("package");
    let result = execute_recorded(
        &f.project,
        &destination,
        CollectionLimits::default(),
        &uuid::Uuid::new_v4().to_string(),
        || false,
        |receipt| {
            // A concurrent writer wins after all private files were verified.
            fs::create_dir(&destination)?;
            fs::write(destination.join("keep"), b"concurrent output")?;
            assert!(receipt.verify(|| false).is_err());
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(Error::Publish(PublishError::OutputExists))
    ));
    assert_eq!(
        fs::read(destination.join("keep")).unwrap(),
        b"concurrent output"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert!(f.stages().is_empty());
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
}
