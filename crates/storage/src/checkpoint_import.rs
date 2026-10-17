//! Atomic metadata import. The host must verify bundle file hashes and all
//! historical asset bytes before preparing this batch; no file I/O holds a DB lock.
use crate::{PortableCheckpoint, RecoveryStore, StorageError, checkpoint_validation};
use rusqlite::{OptionalExtension, params};
use std::{collections::HashSet, path::Path};
use sungeki_project::Checkpoint;

pub struct PreparedCheckpointImport {
    checkpoint: Checkpoint,
    portable: Vec<u8>,
}
impl PreparedCheckpointImport {
    pub fn prepare(
        portable: &PortableCheckpoint,
        root: &Path,
        project_id: &str,
    ) -> Result<Self, crate::DocumentError> {
        Ok(Self {
            checkpoint: portable.resolve(root, project_id)?,
            portable: portable.to_bytes()?,
        })
    }
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CheckpointImportOutcome {
    pub inserted: u32,
    pub unchanged: u32,
    /// Explicit re-import from a moved bundle updates only resolved references.
    pub relocated: u32,
}
fn invalid() -> StorageError {
    StorageError::InvalidVersion
}
impl RecoveryStore {
    /// All-or-nothing, including provenance. Deleted IDs never resurrect, and
    /// ordinary locally created IDs cannot be commandeered by an imported file.
    pub fn import_checkpoints(
        &mut self,
        project_id: &str,
        source_session_id: &str,
        batch: &[PreparedCheckpointImport],
    ) -> Result<CheckpointImportOutcome, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let outcome = write_imports(&tx, project_id, source_session_id, batch)?;
        tx.commit()?;
        Ok(outcome)
    }
    /// Provenance belongs to a live imported checkpoint; deletion removes it.
    pub fn imported_checkpoint_origin(
        &self,
        id: &str,
    ) -> Result<Option<crate::CheckpointOrigin>, StorageError> {
        let bytes: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT portable FROM checkpoint_imports WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        bytes
            .map(|b| {
                PortableCheckpoint::from_bytes(&b)
                    .map(|p| p.origin)
                    .map_err(|_| invalid())
            })
            .transpose()
    }
}

pub(crate) fn write_imports(
    tx: &rusqlite::Transaction<'_>,
    project_id: &str,
    source_session_id: &str,
    batch: &[PreparedCheckpointImport],
) -> Result<CheckpointImportOutcome, StorageError> {
    if batch.len() > 127 || source_session_id.is_empty() {
        return Err(invalid());
    }
    let mut ids = HashSet::new();
    let mut total = 0usize;
    for item in batch {
        checkpoint_validation::validate(&item.checkpoint).map_err(|_| invalid())?;
        if item.checkpoint.project.id != project_id || !ids.insert(&item.checkpoint.summary.id) {
            return Err(invalid());
        }
        total = total
            .checked_add(item.portable.len())
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or_else(invalid)?;
    }
    let mut outcome = CheckpointImportOutcome::default();
    for item in batch {
        let c = &item.checkpoint;
        let deleted: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM deleted_checkpoints WHERE id=?1)",
            [&c.summary.id],
            |r| r.get(0),
        )?;
        if deleted {
            return Err(StorageError::Conflict);
        }
        let old: Option<Vec<u8>> = tx
            .query_row(
                "SELECT snapshot FROM checkpoints WHERE id=?1",
                [&c.summary.id],
                |r| r.get(0),
            )
            .optional()?;
        let origin: Option<(Vec<u8>, String)> = tx
            .query_row(
                "SELECT portable,runtime_hash FROM checkpoint_imports WHERE id=?1",
                [&c.summary.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let snapshot = serde_json::to_vec(c).map_err(|_| invalid())?;
        match (old, origin) {
            (None, None) => {
                tx.execute("INSERT INTO checkpoints(id,project_id,source_session_id,snapshot) VALUES(?1,?2,?3,?4)", params![c.summary.id, project_id, source_session_id, snapshot])?;
                tx.execute(
                    "INSERT INTO checkpoint_imports(id,portable,runtime_hash) VALUES(?1,?2,?3)",
                    params![c.summary.id, item.portable, c.summary.content_hash],
                )?;
                outcome.inserted += 1;
            }
            (Some(old), Some((portable, runtime_hash))) => {
                if portable != item.portable {
                    return Err(StorageError::Conflict);
                }
                let old: Checkpoint = serde_json::from_slice(&old).map_err(|_| invalid())?;
                checkpoint_validation::validate(&old).map_err(|_| invalid())?;
                if old.summary.content_hash != runtime_hash {
                    return Err(StorageError::Conflict);
                }
                // The persisted import must still match its original logical snapshot.
                let mut normalized = old.clone();
                normalized
                    .project
                    .assets
                    .iter_mut()
                    .zip(&c.project.assets)
                    .for_each(|(a, target)| a.locator = target.locator.clone());
                normalized.asset_manifest = checkpoint_validation::manifest(&normalized.project);
                normalized.summary.content_hash =
                    crate::project_hash(&normalized.project).map_err(|_| invalid())?;
                if normalized != *c {
                    return Err(StorageError::Conflict);
                }
                if old == *c {
                    outcome.unchanged += 1;
                } else {
                    tx.execute(
                        "UPDATE checkpoints SET snapshot=?2 WHERE id=?1",
                        params![c.summary.id, snapshot],
                    )?;
                    tx.execute(
                        "UPDATE checkpoint_imports SET runtime_hash=?2 WHERE id=?1",
                        params![c.summary.id, c.summary.content_hash],
                    )?;
                    outcome.relocated += 1;
                }
            }
            _ => return Err(StorageError::Conflict),
        }
    }
    Ok(outcome)
}
