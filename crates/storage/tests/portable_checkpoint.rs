use std::{fs, path::PathBuf};
use sungeki_project::{AssetLocator, Checkpoint, CheckpointAsset, CheckpointSummary, Project};
use sungeki_storage::*;
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
struct Fixture {
    root: PathBuf,
    source: Checkpoint,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("portable-checkpoint-{}", id()));
        fs::create_dir_all(root.join("assets")).unwrap();
        let root = root.canonicalize().unwrap();
        let project: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let source = Checkpoint {
            summary: CheckpointSummary {
                id: id(),
                project_id: project.id.clone(),
                name: "図を追加する前".into(),
                created_at: 1234,
                source_revision: 17,
                content_hash: project_hash(&project).unwrap(),
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
        };
        Self { root, source }
    }
    fn portable(&self) -> PortableCheckpoint {
        PortableCheckpoint::new(&self.source, &id(), &id()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn relocation_preserves_snapshot_and_origin_without_reusing_ids() {
    let mut f = Fixture::new();
    let original = f.source.clone();
    let portable = f.portable();
    let bytes = portable.to_bytes().unwrap();
    let portable = PortableCheckpoint::from_bytes(&bytes).unwrap();
    assert_eq!(portable.origin.checkpoint_id, original.summary.id);
    assert_eq!(portable.origin.content_hash, original.summary.content_hash);
    assert_ne!(portable.id, original.summary.id);
    assert_ne!(portable.project.id, original.project.id);
    let before = portable.resolve(&f.root, &portable.project.id).unwrap();
    let moved = f.root.with_extension("moved");
    fs::rename(&f.root, &moved).unwrap();
    f.root = moved;
    let after = portable.resolve(&f.root, &portable.project.id).unwrap();
    assert_eq!(before.summary.id, after.summary.id);
    assert_ne!(before.summary.content_hash, after.summary.content_hash);
    assert_eq!(bytes, portable.to_bytes().unwrap());
    assert_eq!(after.summary.name, original.summary.name);
    assert_eq!(after.summary.source_revision, 17);
    assert_eq!(after.asset_manifest.len(), original.project.assets.len());
    let mut restored = after.project.clone();
    restored.id = original.project.id.clone();
    for (asset, old) in restored.assets.iter_mut().zip(&original.project.assets) {
        assert!(
            matches!(&asset.locator, AssetLocator::Absolute { path } if PathBuf::from(path).starts_with(&f.root))
        );
        asset.locator = old.locator.clone();
    }
    assert_eq!(restored, original.project); // clips/scenes/credits/probes remain exact.
    assert_eq!(f.source, original);
    let mut store = RecoveryStore::open(&f.root.join("db")).unwrap();
    store
        .create_checkpoint(&StoredCheckpoint {
            source_session_id: "import-test".into(),
            checkpoint: after.clone(),
        })
        .unwrap();
    assert_eq!(
        store
            .checkpoint(&after.summary.id)
            .unwrap()
            .unwrap()
            .checkpoint,
        after
    );
}
#[test]
fn rejects_manifest_mismatch_and_reusing_source_identity() {
    let mut f = Fixture::new();
    assert!(PortableCheckpoint::new(&f.source, &f.source.summary.id, &id()).is_err());
    assert!(PortableCheckpoint::new(&f.source, &id(), &f.source.project.id).is_err());
    f.source.asset_manifest.clear();
    assert!(PortableCheckpoint::new(&f.source, &id(), &id()).is_err());
    let mut store = RecoveryStore::open(&f.root.join("db")).unwrap();
    assert!(
        store
            .create_checkpoint(&StoredCheckpoint {
                source_session_id: "test".into(),
                checkpoint: f.source.clone()
            })
            .is_err()
    );
}
#[test]
fn rejects_tampering_wrong_owner_absolute_and_escaping_paths() {
    let f = Fixture::new();
    let portable = f.portable();
    assert!(portable.resolve(&f.root, &id()).is_err());
    let mut modified = portable.clone();
    modified.project.title.push_str("改変");
    assert!(modified.to_bytes().is_err());
    for path in [
        "../outside.psd",
        "/outside.psd",
        "assets/../outside.psd",
        "assets/other.psd",
    ] {
        let mut modified = portable.clone();
        modified.project.assets[0].locator = AssetLocator::ProjectRelative { path: path.into() };
        // Recomputing the hash must not turn an invalid path into a valid package.
        use sha2::{Digest, Sha256};
        modified.project_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&modified.project).unwrap())
        );
        assert!(PortableCheckpoint::from_bytes(&serde_json::to_vec(&modified).unwrap()).is_err());
    }
    let mut value = serde_json::to_value(&portable).unwrap();
    value["unexpected"] = true.into();
    assert!(PortableCheckpoint::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn rejects_symlinks_in_collection_references_and_invalid_metadata() {
    let f = Fixture::new();
    let portable = f.portable();
    fs::remove_dir(f.root.join("assets")).unwrap();
    std::os::unix::fs::symlink(&f.root, f.root.join("assets")).unwrap();
    assert!(portable.resolve(&f.root, &portable.project.id).is_err());
    for (key, value) in [
        ("version", 2.into()),
        ("name", " ".into()),
        ("created_at", 9_007_199_254_740_992u64.into()),
    ] {
        let mut data = serde_json::to_value(&portable).unwrap();
        data[key] = value;
        assert!(PortableCheckpoint::from_bytes(&serde_json::to_vec(&data).unwrap()).is_err());
    }
}

#[test]
fn stored_corruption_is_rejected_on_read_and_list() {
    let f = Fixture::new();
    let db = f.root.join("db");
    let mut store = RecoveryStore::open(&db).unwrap();
    store
        .create_checkpoint(&StoredCheckpoint {
            source_session_id: "test".into(),
            checkpoint: f.source.clone(),
        })
        .unwrap();
    let mut corrupt = f.source.clone();
    corrupt.asset_manifest[0].content_hash = None;
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE checkpoints SET snapshot=?1 WHERE id=?2",
        rusqlite::params![serde_json::to_vec(&corrupt).unwrap(), corrupt.summary.id,],
    )
    .unwrap();
    assert!(store.checkpoint(&corrupt.summary.id).is_err());
    assert!(
        store
            .list_checkpoints(&corrupt.project.id, None, 10)
            .is_err()
    );
}

#[test]
fn hash_consistent_but_invalid_project_is_rejected_before_resolution() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let mut portable = f.portable();
    portable.project.canvas.width = 0;
    portable.project_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&portable.project).unwrap())
    );
    assert!(PortableCheckpoint::from_bytes(&serde_json::to_vec(&portable).unwrap()).is_err());
}
