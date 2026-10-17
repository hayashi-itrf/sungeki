use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use sungeki_storage::*;
struct Fixture {
    root: PathBuf,
    source: PathBuf,
    bytes: Vec<u8>,
    hash: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("collection-copy-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let source = root.join("source.bin");
        let bytes = vec![42; 400_000];
        fs::write(&source, &bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        Self {
            root,
            source,
            bytes,
            hash,
        }
    }
    fn stage(&self) -> CollectionStage {
        CollectionStage::new(&self.root.join("package")).unwrap()
    }
    fn populate(&self, stage: &CollectionStage) {
        fs::create_dir(stage.path().join("assets")).unwrap();
        fs::write(stage.path().join("project.vvm.json"), "{}").unwrap();
        fs::write(stage.path().join("manifest.json"), "{}").unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn captures_independent_bytes_rejects_old_observations_and_never_overwrites() {
    let f = Fixture::new();
    let stage = f.stage();
    f.populate(&stage);
    let output = stage.path().join("assets/source.bin");
    copy_collection_asset(&f.source, &output, &f.hash, f.bytes.len() as u64, || false).unwrap();
    fs::write(&f.source, vec![43; f.bytes.len()]).unwrap();
    assert_eq!(fs::read(&output).unwrap(), f.bytes);
    assert!(matches!(
        copy_collection_asset(&f.source, &output, &f.hash, f.bytes.len() as u64, || false),
        Err(PublishError::OutputExists)
    ));
    let bad = stage.path().join("assets/changed.bin");
    assert!(matches!(
        copy_collection_asset(&f.source, &bad, &f.hash, f.bytes.len() as u64, || false),
        Err(PublishError::Changed)
    ));
    assert!(!bad.exists());
    let committed = stage.commit(|| false).unwrap();
    assert_eq!(
        fs::read(committed.join("assets/source.bin")).unwrap(),
        f.bytes
    );
}
#[test]
fn cancellation_removes_partial_copies_and_never_publishes_a_directory() {
    let f = Fixture::new();
    let stage = f.stage();
    f.populate(&stage);
    let path = stage.path().to_owned();
    let output = path.join("assets/cancel.bin");
    let mut calls = 0;
    assert!(matches!(
        copy_collection_asset(&f.source, &output, &f.hash, f.bytes.len() as u64, || {
            calls += 1;
            calls == 4
        }),
        Err(PublishError::Cancelled)
    ));
    assert!(!output.exists());
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(matches!(
        stage.commit(|| true),
        Err(PublishError::Cancelled)
    ));
    assert!(!path.exists());
    assert!(!f.root.join("package").exists());
}
#[test]
fn destination_created_during_commit_is_preserved_even_when_empty() {
    for populated in [false, true] {
        let f = Fixture::new();
        let stage = f.stage();
        f.populate(&stage);
        let path = stage.path().to_owned();
        let mut once = false;
        let result = stage.commit(|| {
            if !once {
                once = true;
                fs::create_dir(f.root.join("package")).unwrap();
                if populated {
                    fs::write(f.root.join("package/keep"), "kept").unwrap();
                }
            }
            false
        });
        assert!(matches!(result, Err(PublishError::OutputExists)));
        assert!(!path.exists());
        assert!(f.root.join("package").is_dir());
        if populated {
            assert_eq!(
                fs::read_to_string(f.root.join("package/keep")).unwrap(),
                "kept"
            );
        }
    }
}
#[test]
fn replacement_stage_and_symlink_payloads_are_never_deleted_or_published() {
    let f = Fixture::new();
    let stage = f.stage();
    let path = stage.path().to_owned();
    fs::rename(&path, f.root.join("moved-stage")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), "kept").unwrap();
    assert!(matches!(stage.commit(|| false), Err(PublishError::Changed)));
    assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "kept");
    let stage = f.stage();
    f.populate(&stage);
    std::os::unix::fs::symlink(&f.source, stage.path().join("assets/link")).unwrap();
    assert!(matches!(stage.commit(|| false), Err(PublishError::Changed)));
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(!f.root.join("package").exists());
}
#[test]
fn published_project_resolves_only_packaged_copy_after_original_is_removed() {
    let f = Fixture::new();
    let stage = f.stage();
    f.populate(&stage);
    let target = stage.path().join("assets/source.psd");
    copy_collection_asset(&f.source, &target, &f.hash, f.bytes.len() as u64, || false).unwrap();
    // Byte identity and document relocation fixture; not a PSD decoding test.
    let mut p: sungeki_project::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    p.clips.clear();
    p.assets[0].locator = sungeki_project::AssetLocator::Absolute {
        path: target.to_str().unwrap().into(),
    };
    p.assets[0].content_hash = Some(f.hash.clone());
    let sungeki_project::AssetProbe::Psd(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    probe.sha256 = f.hash.clone();
    probe.byte_length = f.bytes.len() as u64;
    fs::remove_file(stage.path().join("project.vvm.json")).unwrap();
    save_document(&stage.path().join("project.vvm.json"), &p, None).unwrap();
    let result = stage.commit(|| false).unwrap();
    fs::remove_file(&f.source).unwrap();
    let moved = f.root.join("moved-package");
    fs::rename(result, &moved).unwrap();
    let loaded = load_document(&moved.join("project.vvm.json")).unwrap();
    assert_eq!(
        loaded.project.assets[0].locator,
        sungeki_project::AssetLocator::Absolute {
            path: moved.join("assets/source.psd").to_str().unwrap().into()
        }
    );
    assert_eq!(fs::read(moved.join("assets/source.psd")).unwrap(), f.bytes);
}

#[test]
fn original_hardlinks_are_not_accepted_as_collected_copies() {
    let f = Fixture::new();
    let stage = f.stage();
    f.populate(&stage);
    fs::hard_link(&f.source, stage.path().join("assets/link.bin")).unwrap();
    assert!(matches!(stage.commit(|| false), Err(PublishError::Changed)));
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(!f.root.join("package").exists());
}
