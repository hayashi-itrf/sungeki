use std::{fs, path::PathBuf};
use sungeki_project::{Checkpoint, CheckpointAsset, CheckpointSummary, Project};
use sungeki_storage::*;
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
struct Fixture {
    root: PathBuf,
    portable: PortableCheckpoint,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("checkpoint-import-{}", id()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let project: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let source = Checkpoint {
            summary: CheckpointSummary {
                id: id(),
                project_id: project.id.clone(),
                name: "保存時点".into(),
                created_at: 1,
                source_revision: 3,
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
        Self {
            root,
            portable: PortableCheckpoint::new(&source, &id(), &id()).unwrap(),
        }
    }
    fn batch(&self, portable: &PortableCheckpoint) -> PreparedCheckpointImport {
        PreparedCheckpointImport::prepare(portable, &self.root, &self.portable.project.id).unwrap()
    }
    fn store(&self) -> RecoveryStore {
        RecoveryStore::open(&self.root.join("db")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn repeated_import_survives_restart_and_explicit_relocation_without_duplicates() {
    let mut f = Fixture::new();
    let pid = f.portable.project.id.clone();
    let mut store = f.store();
    assert_eq!(
        store
            .import_checkpoints(&pid, "s", &[f.batch(&f.portable)])
            .unwrap()
            .inserted,
        1
    );
    let original = store
        .checkpoint(&f.portable.id)
        .unwrap()
        .unwrap()
        .checkpoint;
    drop(store);
    let mut store = f.store();
    assert_eq!(
        store
            .import_checkpoints(&pid, "s2", &[f.batch(&f.portable)])
            .unwrap()
            .unchanged,
        1
    );
    assert_eq!(
        store.imported_checkpoint_origin(&f.portable.id).unwrap(),
        Some(f.portable.origin.clone())
    );
    drop(store);
    let moved = f.root.with_extension("moved");
    fs::rename(&f.root, &moved).unwrap();
    f.root = moved;
    let mut store = f.store();
    assert_eq!(
        store
            .import_checkpoints(&pid, "s3", &[f.batch(&f.portable)])
            .unwrap()
            .relocated,
        1
    );
    let relocated = store
        .checkpoint(&f.portable.id)
        .unwrap()
        .unwrap()
        .checkpoint;
    assert_ne!(
        relocated.summary.content_hash,
        original.summary.content_hash
    );
    assert_eq!(relocated.summary.id, original.summary.id);
    assert_eq!(store.list_checkpoints(&pid, None, 10).unwrap().len(), 1);
    assert_eq!(
        store
            .import_checkpoints(&pid, "s3", &[f.batch(&f.portable)])
            .unwrap()
            .unchanged,
        1
    );
}
#[test]
fn conflict_rolls_back_entire_batch_and_provenance() {
    let f = Fixture::new();
    let mut store = f.store();
    let pid = &f.portable.project.id;
    store
        .import_checkpoints(pid, "s", &[f.batch(&f.portable)])
        .unwrap();
    let mut new = f.portable.clone();
    new.id = id();
    let mut conflict = f.portable.clone();
    conflict.name = "別の名前".into();
    conflict.project.title = "同じIDの別作品内容".into();
    use sha2::{Digest, Sha256};
    conflict.project_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&conflict.project).unwrap())
    );
    assert!(matches!(
        store.import_checkpoints(pid, "s", &[f.batch(&new), f.batch(&conflict)]),
        Err(StorageError::Conflict)
    ));
    assert!(store.checkpoint(&new.id).unwrap().is_none());
    assert!(store.imported_checkpoint_origin(&new.id).unwrap().is_none());
    assert_eq!(
        store
            .checkpoint(&f.portable.id)
            .unwrap()
            .unwrap()
            .checkpoint
            .summary
            .name,
        "保存時点"
    );
}
#[test]
fn deletion_reserves_imported_id_and_local_checkpoint_cannot_be_overwritten() {
    let f = Fixture::new();
    let mut store = f.store();
    let pid = &f.portable.project.id;
    store
        .import_checkpoints(pid, "s", &[f.batch(&f.portable)])
        .unwrap();
    store.delete_checkpoint(&f.portable.id, pid).unwrap();
    assert!(
        store
            .imported_checkpoint_origin(&f.portable.id)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        store.import_checkpoints(pid, "s", &[f.batch(&f.portable)]),
        Err(StorageError::Conflict)
    ));
    assert!(store.checkpoint(&f.portable.id).unwrap().is_none());
    let mut other = f.portable.clone();
    other.id = id();
    store
        .create_checkpoint(&StoredCheckpoint {
            source_session_id: "local".into(),
            checkpoint: other.resolve(&f.root, pid).unwrap(),
        })
        .unwrap();
    assert!(matches!(
        store.import_checkpoints(pid, "s", &[f.batch(&other)]),
        Err(StorageError::Conflict)
    ));
    assert!(
        store
            .imported_checkpoint_origin(&other.id)
            .unwrap()
            .is_none()
    );
}
#[test]
fn wrong_owner_duplicate_ids_and_corrupt_stored_snapshot_are_rejected() {
    let f = Fixture::new();
    let mut store = f.store();
    let pid = &f.portable.project.id;
    assert!(
        store
            .import_checkpoints(&id(), "s", &[f.batch(&f.portable)])
            .is_err()
    );
    assert!(
        store
            .import_checkpoints(pid, "s", &[f.batch(&f.portable), f.batch(&f.portable)])
            .is_err()
    );
    store
        .import_checkpoints(pid, "s", &[f.batch(&f.portable)])
        .unwrap();
    let connection = rusqlite::Connection::open(f.root.join("db")).unwrap();
    connection
        .execute("UPDATE checkpoints SET snapshot=X'00'", [])
        .unwrap();
    assert!(
        store
            .import_checkpoints(pid, "s", &[f.batch(&f.portable)])
            .is_err()
    );
}
#[test]
fn v14_migration_preserves_local_history_and_does_not_claim_it_as_imported() {
    let f = Fixture::new();
    let pid = &f.portable.project.id;
    let mut store = f.store();
    let checkpoint = f.portable.resolve(&f.root, pid).unwrap();
    store
        .create_checkpoint(&StoredCheckpoint {
            source_session_id: "s".into(),
            checkpoint: checkpoint.clone(),
        })
        .unwrap();
    drop(store);
    let raw = rusqlite::Connection::open(f.root.join("db")).unwrap();
    raw.execute_batch("DROP TABLE checkpoint_imports; PRAGMA user_version=14;")
        .unwrap();
    drop(raw);
    let store = f.store();
    assert_eq!(
        store
            .checkpoint(&checkpoint.summary.id)
            .unwrap()
            .unwrap()
            .checkpoint,
        checkpoint
    );
    assert!(
        store
            .imported_checkpoint_origin(&checkpoint.summary.id)
            .unwrap()
            .is_none()
    );
}

#[path = "checkpoint_import/open.rs"]
mod open;
