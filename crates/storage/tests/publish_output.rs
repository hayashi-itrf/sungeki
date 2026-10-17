use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use sungeki_storage::{PublishError, publish_output};

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    target: PathBuf,
    bytes: Vec<u8>,
    hash: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("vvm-publish-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.mp4");
        let target = root.join("完成動画.mp4");
        // Several chunks; this is a storage fixture, not a valid video.
        let bytes: Vec<_> = (0..400_000).map(|i| (i % 251) as u8).collect();
        fs::write(&source, &bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        Self {
            root,
            source,
            target,
            bytes,
            hash,
        }
    }

    fn stage(&self) -> Option<PathBuf> {
        fs::read_dir(&self.root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "tmp"))
    }

    fn publish(
        &self,
        cancel: impl FnMut() -> bool,
    ) -> Result<sungeki_storage::PublishedOutput, PublishError> {
        publish_output(
            &self.source,
            &self.target,
            &self.hash,
            self.bytes.len() as u64,
            cancel,
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn copy_and_retry_preserve_original_and_published_inode() {
    let f = Fixture::new();
    let original_inode = fs::metadata(&f.source).unwrap().ino();
    let result = f.publish(|| false).unwrap();
    assert_eq!(result.path, f.target.canonicalize().unwrap());
    assert!(!result.already_present);
    let inode = fs::metadata(&f.target).unwrap().ino();
    assert_ne!(inode, original_inode);
    assert!(f.publish(|| false).unwrap().already_present);
    assert_eq!(fs::metadata(&f.target).unwrap().ino(), inode);
    assert_eq!(fs::read(&f.target).unwrap(), f.bytes);
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(f.stage().is_none());
}

#[test]
fn source_itself_and_existing_hard_link_are_not_independent_publications() {
    let f = Fixture::new();
    assert!(matches!(
        publish_output(&f.source, &f.source, &f.hash, f.bytes.len() as u64, || {
            false
        }),
        Err(PublishError::OutputExists)
    ));
    fs::hard_link(&f.source, &f.target).unwrap();
    assert!(matches!(
        f.publish(|| false),
        Err(PublishError::OutputExists)
    ));
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert_eq!(fs::read(&f.target).unwrap(), f.bytes);
    assert!(f.stage().is_none());
}

#[test]
fn conflicting_file_directory_and_symlink_are_never_replaced() {
    let f = Fixture::new();
    fs::write(&f.target, b"existing movie").unwrap();
    assert!(matches!(
        f.publish(|| false),
        Err(PublishError::OutputExists)
    ));
    assert_eq!(fs::read(&f.target).unwrap(), b"existing movie");
    fs::remove_file(&f.target).unwrap();
    fs::create_dir(&f.target).unwrap();
    assert!(matches!(
        f.publish(|| false),
        Err(PublishError::OutputExists)
    ));
    assert!(f.target.is_dir());
    fs::remove_dir(&f.target).unwrap();
    symlink(&f.source, &f.target).unwrap();
    assert!(matches!(
        f.publish(|| false),
        Err(PublishError::OutputExists)
    ));
    assert!(fs::symlink_metadata(&f.target).unwrap().is_symlink());
    assert!(f.stage().is_none());
}

#[test]
fn changed_hash_size_and_symlink_source_are_rejected() {
    let f = Fixture::new();
    for (hash, size) in [("0".repeat(64), f.bytes.len() as u64), (f.hash.clone(), 1)] {
        assert!(matches!(
            publish_output(&f.source, &f.target, &hash, size, || false),
            Err(PublishError::Changed)
        ));
        assert!(!f.target.exists());
    }
    let alias = f.root.join("alias.mp4");
    symlink(&f.source, &alias).unwrap();
    assert!(matches!(
        publish_output(&alias, &f.target, &f.hash, f.bytes.len() as u64, || false),
        Err(PublishError::Changed)
    ));
    assert!(f.stage().is_none());
}

#[test]
fn cancellation_before_and_during_copy_leaves_no_output() {
    let f = Fixture::new();
    assert!(matches!(f.publish(|| true), Err(PublishError::Cancelled)));
    assert!(f.stage().is_none());
    assert!(matches!(
        f.publish(|| f
            .stage()
            .is_some_and(|p| fs::metadata(p).unwrap().len() > 0)),
        Err(PublishError::Cancelled)
    ));
    assert!(!f.target.exists());
    assert!(f.stage().is_none());
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
}

#[test]
fn source_path_replacement_during_copy_is_detected() {
    let f = Fixture::new();
    let mut replaced = false;
    let result = f.publish(|| {
        if !replaced
            && f.stage()
                .is_some_and(|p| fs::metadata(p).unwrap().len() > 0)
        {
            fs::rename(&f.source, f.root.join("previous.mp4")).unwrap();
            fs::write(&f.source, &f.bytes).unwrap();
            replaced = true;
        }
        false
    });
    assert!(replaced);
    assert!(matches!(result, Err(PublishError::Changed)));
    assert!(!f.target.exists());
    assert!(f.stage().is_none());
}

#[test]
fn in_place_source_edit_and_cancel_at_commit_boundary_are_rejected() {
    let f = Fixture::new();
    let mut edited = false;
    let result = f.publish(|| {
        if !edited
            && f.stage()
                .is_some_and(|p| fs::metadata(p).unwrap().len() > 0)
        {
            fs::write(&f.source, vec![0; f.bytes.len()]).unwrap();
            edited = true;
        }
        false
    });
    assert!(edited);
    assert!(matches!(result, Err(PublishError::Changed)));
    assert!(!f.target.exists());
    assert!(f.stage().is_none());
    fs::write(&f.source, &f.bytes).unwrap();
    let mut complete_checks = 0;
    let result = f.publish(|| {
        if f.stage()
            .is_some_and(|p| fs::metadata(p).unwrap().len() == f.bytes.len() as u64)
        {
            complete_checks += 1;
        }
        // First is the EOF read, second is after syncing the stage, before publication.
        complete_checks == 2
    });
    assert_eq!(complete_checks, 2);
    assert!(matches!(result, Err(PublishError::Cancelled)));
    assert!(!f.target.exists());
    assert!(f.stage().is_none());
}

#[test]
fn invalid_destination_or_fingerprint_does_not_create_files() {
    let f = Fixture::new();
    for target in [
        Path::new("relative.mp4").to_owned(),
        f.root.join("movie.mov"),
    ] {
        assert!(matches!(
            publish_output(&f.source, &target, &f.hash, f.bytes.len() as u64, || false),
            Err(PublishError::InvalidDestination)
        ));
    }
    assert!(matches!(
        publish_output(&f.source, &f.target, "bad", 1, || false),
        Err(PublishError::InvalidFingerprint)
    ));
    assert!(f.stage().is_none());
}

#[test]
fn concurrent_different_outputs_have_exactly_one_winner() {
    let f = Fixture::new();
    let other = f.root.join("other.mp4");
    let other_bytes = vec![7; 400_000];
    fs::write(&other, &other_bytes).unwrap();
    let other_hash = format!("{:x}", Sha256::digest(&other_bytes));
    let barrier = std::sync::Barrier::new(2);
    let run = |source: &Path, hash: &str| {
        let mut waiting = true;
        publish_output(source, &f.target, hash, 400_000, || {
            if waiting {
                waiting = false;
                barrier.wait();
            }
            false
        })
    };
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| run(&f.source, &f.hash));
        let b = scope.spawn(|| run(&other, &other_hash));
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(PublishError::OutputExists)))
            .count(),
        1
    );
    let actual = fs::read(&f.target).unwrap();
    assert!(actual == f.bytes || actual == other_bytes);
    assert!(f.stage().is_none());
}

#[test]
fn cleanup_does_not_remove_replacement_of_own_stage() {
    let f = Fixture::new();
    let mut replaced = None;
    let result = f.publish(|| {
        if replaced.is_none()
            && let Some(path) = f.stage()
            && fs::metadata(&path).unwrap().len() > 0
        {
            fs::rename(&path, f.root.join("detached-stage")).unwrap();
            fs::write(&path, b"foreign file").unwrap();
            replaced = Some(path);
        }
        false
    });
    assert!(matches!(result, Err(PublishError::Changed)));
    assert_eq!(fs::read(replaced.unwrap()).unwrap(), b"foreign file");
    assert!(!f.target.exists());
}
