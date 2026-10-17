use super::*;
use crate::collection_execute::history::FrozenCheckpoint;
use sungeki_project::{Checkpoint, CheckpointAsset, CheckpointSummary};
use sungeki_storage::PortableCheckpoint;
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn frozen(project: Project) -> FrozenCheckpoint {
    FrozenCheckpoint {
        collected_id: id(),
        source: Checkpoint {
            summary: CheckpointSummary {
                id: id(),
                project_id: project.id.clone(),
                name: "図を変更する前".into(),
                created_at: 1000,
                source_revision: 1,
                content_hash: sungeki_storage::project_hash(&project).unwrap(),
            },
            asset_manifest: project
                .assets
                .iter()
                .map(|a| CheckpointAsset {
                    asset_id: a.id.clone(),
                    locator: a.locator.clone(),
                    content_hash: a.content_hash.clone(),
                })
                .collect(),
            project,
        },
    }
}
pub(super) fn historical(f: &Fixture) -> (FrozenCheckpoint, PathBuf) {
    let path = f.root.join("old.psd");
    let bytes = vec![19; 2048];
    fs::write(&path, &bytes).unwrap();
    let mut project = f.project.clone();
    project.assets.truncate(1);
    let asset = &mut project.assets[0];
    asset.locator = AssetLocator::Absolute {
        path: path.to_str().unwrap().into(),
    };
    let hash = format!("{:x}", Sha256::digest(&bytes));
    asset.content_hash = Some(hash.clone());
    let sungeki_project::AssetProbe::Psd(probe) = &mut asset.probe else {
        panic!()
    };
    probe.sha256 = hash;
    probe.byte_length = bytes.len() as u64;
    (frozen(project), path)
}
#[test]
fn history_only_assets_are_copied_once_and_verified_after_relocation() {
    let f = Fixture::new();
    let (old, old_path) = historical(&f);
    let history = vec![frozen(f.project.clone()), old];
    let original = history.clone();
    let project_id = id();
    let mut receipt = execute_with_history(
        &f.project,
        &history,
        &f.root.join("package"),
        CollectionLimits::default(),
        &project_id,
        || false,
        |receipt| {
            assert!(!receipt.destination.exists());
            assert_eq!(
                receipt
                    .files
                    .iter()
                    .filter(|f| f.path.starts_with("checkpoints/"))
                    .count(),
                2
            );
            Ok(())
        },
    )
    .unwrap();
    receipt.verify(|| false).unwrap();
    assert_eq!(history, original);
    assert_eq!(
        fs::read_dir(receipt.destination.join("assets"))
            .unwrap()
            .count(),
        2
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(receipt.destination.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["checkpoints_included"], true);
    assert_eq!(
        manifest["checkpoints"][1]["credits"][0]["credit"],
        "保持する素材クレジット"
    );
    assert_eq!(
        manifest["checkpoints"][1]["credits"][0]["license_status"],
        "not_verified"
    );
    assert_eq!(
        manifest["checkpoints"][1]["origin"]["checkpoint_id"],
        history[1].source.summary.id
    );
    fs::remove_file(&old_path).unwrap();
    fs::remove_file(&f.source).unwrap();
    let moved = f.root.join("moved");
    fs::rename(&receipt.destination, &moved).unwrap();
    receipt.destination = moved;
    receipt.verify(|| false).unwrap();
    for item in &history {
        let path = receipt
            .destination
            .join(format!("checkpoints/{}.json", item.collected_id));
        let portable = PortableCheckpoint::from_bytes(&fs::read(&path).unwrap()).unwrap();
        let loaded = portable.resolve(&receipt.destination, &project_id).unwrap();
        for asset in loaded.project.assets {
            let AssetLocator::Absolute { path } = asset.locator else {
                panic!()
            };
            assert!(Path::new(&path).starts_with(&receipt.destination));
            assert_eq!(
                format!("{:x}", Sha256::digest(fs::read(path).unwrap())),
                asset.probe.sha256()
            );
            assert_eq!(asset.credit, "保持する素材クレジット");
        }
    }
    let path = receipt
        .destination
        .join(format!("checkpoints/{}.json", history[0].collected_id));
    fs::write(path, b"{}").unwrap();
    assert!(receipt.verify(|| false).is_err());
}
#[test]
fn missing_or_changed_historical_original_prevents_publication() {
    for changed in [false, true] {
        let f = Fixture::new();
        let (old, path) = historical(&f);
        if changed {
            fs::write(path, vec![18; 2048]).unwrap();
        } else {
            fs::remove_file(path).unwrap();
        }
        let result = execute_with_history(
            &f.project,
            &[old],
            &f.root.join("package"),
            CollectionLimits::default(),
            &id(),
            || false,
            |_| panic!("must not record publication"),
        );
        assert!(matches!(result, Err(Error::Api(_))));
        assert!(!f.root.join("package").exists());
        assert!(f.stages().is_empty());
    }
}
#[test]
fn history_identity_collisions_and_foreign_project_are_rejected() {
    let f = Fixture::new();
    let mut one = frozen(f.project.clone());
    let mut two = frozen(f.project.clone());
    two.collected_id = one.source.summary.id.clone();
    assert!(
        execute_with_history(
            &f.project,
            &[one.clone(), two],
            &f.root.join("package"),
            CollectionLimits::default(),
            &id(),
            || false,
            |_| Ok(())
        )
        .is_err()
    );
    one.source.project.id = id();
    one.source.summary.project_id = one.source.project.id.clone();
    one.source.summary.content_hash = sungeki_storage::project_hash(&one.source.project).unwrap();
    assert!(
        execute_with_history(
            &f.project,
            &[one],
            &f.root.join("package"),
            CollectionLimits::default(),
            &id(),
            || false,
            |_| Ok(())
        )
        .is_err()
    );
    assert!(f.stages().is_empty());
}
#[test]
fn replaced_checkpoint_directory_is_rejected_without_deleting_it() {
    let f = Fixture::new();
    let h = frozen(f.project.clone());
    let receipt = execute_with_history(
        &f.project,
        &[h],
        &f.root.join("package"),
        CollectionLimits::default(),
        &id(),
        || false,
        |_| Ok(()),
    )
    .unwrap();
    let path = receipt.destination.join("checkpoints");
    let renamed = receipt.destination.join("kept-history");
    fs::rename(&path, &renamed).unwrap();
    std::os::unix::fs::symlink(&renamed, &path).unwrap();
    assert!(receipt.verify(|| false).is_err());
    assert!(renamed.is_dir());
    assert!(path.is_symlink());
}
