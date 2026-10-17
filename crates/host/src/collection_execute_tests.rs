use super::*;
use sha2::{Digest, Sha256};
use std::cell::Cell;
struct Fixture {
    root: PathBuf,
    source: PathBuf,
    project: Project,
    bytes: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("collect-execute-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let source = root.join("original.psd");
        // File identity fixture; this test does not decode PSD or render a video.
        let bytes = vec![42; 400_000];
        fs::write(&source, &bytes).unwrap();
        let mut project: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        project.clips.clear();
        let asset = &mut project.assets[0];
        asset.locator = AssetLocator::Absolute {
            path: source.to_str().unwrap().into(),
        };
        asset.credit = "保持する素材クレジット".into();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        asset.content_hash = Some(hash.clone());
        let sungeki_project::AssetProbe::Psd(p) = &mut asset.probe else {
            panic!()
        };
        p.sha256 = hash;
        p.byte_length = bytes.len() as u64;
        let mut duplicate = asset.clone();
        duplicate.id = uuid::Uuid::new_v4().to_string();
        project.assets.push(duplicate);
        Self {
            root,
            source,
            project,
            bytes,
        }
    }
    fn stages(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".vvm-collect-")
            })
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn collection_survives_relocation_preserves_credits_and_separates_identity() {
    let f = Fixture::new();
    let before = f.project.clone();
    let mut receipt = execute_recorded(
        &f.project,
        &f.root.join("package"),
        CollectionLimits::default(),
        &uuid::Uuid::new_v4().to_string(),
        || false,
        |_| Ok(()),
    )
    .unwrap();
    let path = receipt.destination.clone();
    receipt.verify(|| false).unwrap();
    assert_eq!(f.project, before);
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(f.stages().is_empty());
    assert_eq!(fs::read_dir(path.join("assets")).unwrap().count(), 1);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["assets"][0]["credit"], "保持する素材クレジット");
    assert_eq!(manifest["assets"][0]["license_status"], "not_verified");
    assert_eq!(manifest["checkpoints_included"], false);
    assert!(manifest["font_identity"].is_null());
    assert_ne!(
        manifest["source_project_id"],
        manifest["collected_project_id"]
    );
    let raw: Project =
        serde_json::from_slice(&fs::read(path.join("project.vvm.json")).unwrap()).unwrap();
    assert!(
        raw.assets
            .iter()
            .all(|a| matches!(a.locator, AssetLocator::ProjectRelative { .. }))
    );
    fs::remove_file(&f.source).unwrap();
    let moved = f.root.join("relocated");
    fs::rename(path, &moved).unwrap();
    assert!(receipt.verify(|| false).is_err());
    // Test relocation explicitly; automatic recovery must retain the recorded path.
    receipt.destination = moved.clone();
    receipt.verify(|| false).unwrap();
    let reopened = sungeki_storage::load_document(&moved.join("project.vvm.json")).unwrap();
    assert_eq!(manifest["project_sha256"], reopened.stamp.hash);
    assert_ne!(reopened.project.id, before.id);
    for a in reopened.project.assets {
        let AssetLocator::Absolute { path } = a.locator else {
            panic!()
        };
        assert!(Path::new(&path).starts_with(&moved));
        assert_eq!(fs::read(path).unwrap(), f.bytes);
    }
    let asset = receipt
        .files
        .iter()
        .find(|file| file.path.starts_with("assets/"))
        .unwrap();
    fs::write(moved.join(&asset.path), vec![43; f.bytes.len()]).unwrap();
    assert!(receipt.verify(|| false).is_err());
}
#[test]
fn changed_input_and_cancel_before_copy_leave_no_completed_or_partial_folder() {
    for change in [false, true] {
        let f = Fixture::new();
        let fired = Cell::new(false);
        let result = execute(
            &f.project,
            &f.root.join("package"),
            CollectionLimits::default(),
            || {
                if !f.stages().is_empty() && !fired.replace(true) {
                    if change {
                        fs::write(&f.source, vec![43; f.bytes.len()]).unwrap();
                    } else {
                        return true;
                    }
                }
                false
            },
        );
        assert!(fired.get());
        assert!(matches!(
            result,
            Err(Error::Publish(
                PublishError::Changed | PublishError::Cancelled
            ))
        ));
        assert!(!f.root.join("package").exists());
        assert!(f.stages().is_empty());
        if !change {
            assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
        }
    }
}
#[test]
fn missing_sources_and_existing_destination_are_preserved() {
    let f = Fixture::new();
    let target = f.root.join("existing");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), b"keep").unwrap();
    assert!(matches!(
        execute(&f.project, &target, CollectionLimits::default(), || false),
        Err(Error::Publish(PublishError::OutputExists))
    ));
    fs::remove_file(&f.source).unwrap();
    assert!(matches!(
        execute(
            &f.project,
            &f.root.join("missing"),
            CollectionLimits::default(),
            || false
        ),
        Err(Error::Api(sungeki_protocol::ApiError {
            code: sungeki_protocol::ErrorCode::AssetMissing,
            ..
        }))
    ));
    assert_eq!(fs::read(target.join("keep")).unwrap(), b"keep");
    assert!(f.stages().is_empty());
    assert!(!f.root.join("missing").exists());
}

#[path = "collection_history_tests.rs"]
mod history_tests;

#[path = "collection_import_tests.rs"]
mod import_tests;

#[test]
fn stage_record_failure_prevents_all_asset_writes_and_publication() {
    let f = Fixture::new();
    let result = execute_journaled(
        &f.project,
        &[],
        &f.root.join("package"),
        CollectionLimits::default(),
        &uuid::Uuid::new_v4().to_string(),
        || false,
        |stage| {
            assert_eq!(
                stage.inspect().unwrap(),
                sungeki_storage::CollectionStagePresence::Owned
            );
            assert_eq!(fs::read_dir(stage.path()).unwrap().count(), 0);
            Err(Error::Io(std::io::Error::other(
                "stage journal persistence failed",
            )))
        },
        |_| panic!("must not publish after failed stage journal"),
    );
    assert!(matches!(result, Err(Error::Io(_))));
    assert!(f.stages().is_empty());
    assert!(!f.root.join("package").exists());
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
}
#[test]
fn registered_video_collection_preserves_metadata_and_verifies_its_receipt() {
    let f = Fixture::new();
    let mut source: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    for (container, extension) in [
        (sungeki_project::VideoContainer::Mov, "mov"),
        (sungeki_project::VideoContainer::Matroska, "mkv"),
    ] {
        source.assets.truncate(1);
        let asset = &mut source.assets[0];
        asset.locator = AssetLocator::Absolute {
            path: f.source.to_str().unwrap().into(),
        };
        let sungeki_project::AssetProbe::Video(probe) = &mut asset.probe else {
            panic!()
        };
        probe.sha256 = format!("{:x}", Sha256::digest(&f.bytes));
        probe.byte_length = f.bytes.len() as u64;
        probe.metadata.container = container;
        asset.content_hash = Some(probe.sha256.clone());
        let mut duplicate = asset.clone();
        duplicate.id = uuid::Uuid::new_v4().to_string();
        source.assets.push(duplicate);
        let before = source.clone();
        let receipt = execute_recorded(
            &source,
            &f.root.join(extension),
            CollectionLimits::default(),
            &uuid::Uuid::new_v4().to_string(),
            || false,
            |_| Ok(()),
        )
        .unwrap();
        receipt.verify(|| false).unwrap();
        assert_eq!(source, before);
        let loaded = sungeki_storage::load_document(&receipt.destination.join("project.vvm.json"))
            .unwrap()
            .project;
        assert_eq!(loaded.assets[0].probe, source.assets[0].probe);
        let entries: Vec<_> = fs::read_dir(receipt.destination.join("assets"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].extension().unwrap(), extension);
        assert_eq!(fs::read(&entries[0]).unwrap(), f.bytes);
    }
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(f.stages().is_empty());
}

#[path = "collection_failure_tests.rs"]
mod failure_tests;
