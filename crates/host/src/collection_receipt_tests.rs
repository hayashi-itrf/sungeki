use super::*;
use crate::collection_execute::{Error, execute_recorded};
use std::cell::RefCell;
use sungeki_assets::CollectionLimits;
use sungeki_project::Project;
struct Fixture {
    root: PathBuf,
    project: Project,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("collection-receipt-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
            project: Project::empty(uuid::Uuid::new_v4().to_string(), "公開回復検証".into()),
        }
    }
    fn run(&self, hook: impl FnOnce(&Receipt) -> Result<(), Error>) -> Result<Receipt, Error> {
        execute_recorded(
            &self.project,
            &self.root.join("output"),
            CollectionLimits::default(),
            &uuid::Uuid::new_v4().to_string(),
            || false,
            hook,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn intent_is_recorded_before_publication_and_verifies_after_reload() {
    let f = Fixture::new();
    let stored = RefCell::new(Vec::new());
    let output = f
        .run(|receipt| {
            assert!(!receipt.destination.exists());
            *stored.borrow_mut() = serde_json::to_vec(receipt).unwrap();
            Ok(())
        })
        .unwrap();
    let recorded: Receipt = serde_json::from_slice(&stored.into_inner()).unwrap();
    assert_eq!(recorded, output);
    recorded.verify(|| false).unwrap();
    // Receipt alone does not imply success; changed bytes must be rejected.
    fs::write(output.destination.join("manifest.json"), "{}").unwrap();
    assert!(recorded.verify(|| false).is_err());
}
#[test]
fn failed_recording_never_publishes_and_recorded_intent_is_not_success() {
    let f = Fixture::new();
    let saved = RefCell::new(None);
    let result = f.run(|receipt| {
        *saved.borrow_mut() = Some(receipt.clone());
        Err(Error::Io(std::io::Error::other(
            "simulated DB write failure",
        )))
    });
    assert!(matches!(result, Err(Error::Io(_))));
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
    assert!(saved.into_inner().unwrap().verify(|| false).is_err());
}
#[test]
fn unsafe_paths_duplicates_and_identity_mismatches_are_rejected() {
    let f = Fixture::new();
    let receipt = f.run(|_| Ok(())).unwrap();
    for path in [
        "../escape",
        "/tmp/escape",
        "assets/../escape",
        "assets/no-hash.png",
    ] {
        let mut bad = receipt.clone();
        bad.files.push(File {
            path: path.into(),
            sha256: "a".repeat(64),
            byte_length: 1,
        });
        assert!(bad.validate().is_err());
    }
    let mut duplicate = receipt.clone();
    duplicate.files.push(duplicate.files[0].clone());
    assert!(duplicate.validate().is_err());
    let mut other = receipt.clone();
    other.collected_project_id = uuid::Uuid::new_v4().to_string();
    assert!(other.verify(|| false).is_err());
    assert!(matches!(
        receipt.verify(|| true),
        Err(Error::Publish(PublishError::Cancelled))
    ));
    receipt.verify(|| false).unwrap();
}
#[test]
fn replaced_folder_symlinks_are_not_accepted_or_removed() {
    let f = Fixture::new();
    let receipt = f.run(|_| Ok(())).unwrap();
    let moved = f.root.join("moved");
    fs::rename(&receipt.destination, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &receipt.destination).unwrap();
    assert!(receipt.verify(|| false).is_err());
    assert!(moved.join("project.vvm.json").is_file());
    fs::remove_file(&receipt.destination).unwrap();
    fs::rename(&moved, &receipt.destination).unwrap();
    fs::rename(receipt.destination.join("assets"), &moved).unwrap();
    std::os::unix::fs::symlink(&moved, receipt.destination.join("assets")).unwrap();
    assert!(receipt.verify(|| false).is_err());
    assert!(moved.is_dir());
}
