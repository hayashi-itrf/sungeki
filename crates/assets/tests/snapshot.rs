use image::{ImageFormat, RgbaImage};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use sungeki_assets::{ProbeError, ProbeLimits, SnapshotError, probe_image, snapshot_image};

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    copy: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("vvm-snapshot-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.png");
        let copy = root.join("job-original.png");
        let mut bytes = Cursor::new(Vec::new());
        RgbaImage::new(8, 5)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        fs::write(&source, bytes.into_inner()).unwrap();
        Self { root, source, copy }
    }
    fn hash(&self) -> String {
        probe_image(&self.source, ProbeLimits::default())
            .unwrap()
            .sha256
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn copy_remains_valid_after_original_is_overwritten_and_removed() {
    let f = Fixture::new();
    let bytes = fs::read(&f.source).unwrap();
    let expected = probe_image(&f.source, ProbeLimits::default()).unwrap();
    let actual =
        snapshot_image(&f.source, &f.copy, &expected.sha256, ProbeLimits::default()).unwrap();
    assert_eq!(actual, expected);
    fs::write(&f.source, b"changed in place").unwrap();
    assert_eq!(fs::read(&f.copy).unwrap(), bytes);
    fs::remove_file(&f.source).unwrap();
    assert_eq!(
        probe_image(&f.copy, ProbeLimits::default()).unwrap(),
        expected
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(&f.copy).unwrap().mode() & 0o777, 0o600);
    }
}

#[test]
fn changed_or_missing_original_never_produces_a_snapshot() {
    let f = Fixture::new();
    let hash = f.hash();
    fs::write(&f.source, b"changed").unwrap();
    assert!(matches!(
        snapshot_image(&f.source, &f.copy, &hash, ProbeLimits::default()),
        Err(SnapshotError::Source(ProbeError::SourceMismatch))
    ));
    assert!(!f.copy.exists());
    fs::remove_file(&f.source).unwrap();
    assert!(matches!(
        snapshot_image(&f.source, &f.copy, &hash, ProbeLimits::default()),
        Err(SnapshotError::Source(ProbeError::Io(_)))
    ));
    assert!(!f.copy.exists());
}

#[test]
fn refuses_existing_files_and_symlinks_without_altering_them() {
    let f = Fixture::new();
    let hash = f.hash();
    fs::write(&f.copy, b"existing artifact").unwrap();
    assert!(snapshot_image(&f.source, &f.copy, &hash, ProbeLimits::default()).is_err());
    assert_eq!(fs::read(&f.copy).unwrap(), b"existing artifact");
    #[cfg(unix)]
    {
        fs::remove_file(&f.copy).unwrap();
        std::os::unix::fs::symlink(&f.source, &f.copy).unwrap();
        assert!(snapshot_image(&f.source, &f.copy, &hash, ProbeLimits::default()).is_err());
        assert_eq!(f.hash(), hash);
        assert!(
            fs::symlink_metadata(&f.copy)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn enforces_capture_and_decode_limits_before_creating_output() {
    let f = Fixture::new();
    for limits in [
        ProbeLimits {
            max_file_bytes: 1,
            ..ProbeLimits::default()
        },
        ProbeLimits {
            max_pixels: 1,
            ..ProbeLimits::default()
        },
    ] {
        assert!(matches!(
            snapshot_image(&f.source, &f.copy, &f.hash(), limits),
            Err(SnapshotError::Source(ProbeError::TooLarge))
        ));
        assert!(!f.copy.exists());
    }
    assert!(
        snapshot_image(
            &f.source,
            Path::new("relative.png"),
            &f.hash(),
            ProbeLimits::default()
        )
        .is_err()
    );
    assert!(
        snapshot_image(
            &f.source,
            &f.root.join("missing/child.png"),
            &f.hash(),
            ProbeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn destination_failure_is_distinct_from_a_missing_original() {
    let f = Fixture::new();
    let result = snapshot_image(
        &f.source,
        &f.root.join("absent/output"),
        &f.hash(),
        ProbeLimits::default(),
    );
    assert!(
        matches!(result, Err(SnapshotError::Destination(e)) if e.kind() == std::io::ErrorKind::NotFound)
    );
    assert!(f.source.exists());
}
