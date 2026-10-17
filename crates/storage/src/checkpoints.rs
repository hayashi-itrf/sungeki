use crate::{RecoveryStore, StorageError, checkpoint_validation};
use rusqlite::{OptionalExtension, params};
use sungeki_project::{Checkpoint, CheckpointSummary};

pub struct StoredCheckpoint {
    pub source_session_id: String,
    pub checkpoint: Checkpoint,
}
fn invalid() -> StorageError {
    StorageError::InvalidVersion
}
impl RecoveryStore {
    pub fn deleted_checkpoint_project(&self, id: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT project_id FROM deleted_checkpoints WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
    // A tombstone reserves the ID permanently, preventing delayed create retries from resurrecting it.
    pub fn delete_checkpoint(&mut self, id: &str, project_id: &str) -> Result<bool, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owner: Option<String> = tx
            .query_row(
                "SELECT project_id FROM deleted_checkpoints WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(owner) = owner {
            return if owner == project_id {
                Ok(false)
            } else {
                Err(StorageError::Conflict)
            };
        }
        let changed = tx.execute(
            "DELETE FROM checkpoints WHERE id=?1 AND project_id=?2",
            params![id, project_id],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict);
        }
        tx.execute("DELETE FROM checkpoint_imports WHERE id=?1", [id])?;
        tx.execute(
            "INSERT INTO deleted_checkpoints(id,project_id) VALUES(?1,?2)",
            params![id, project_id],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn create_checkpoint(&mut self, value: &StoredCheckpoint) -> Result<(), StorageError> {
        let c = &value.checkpoint;
        if self.deleted_checkpoint_project(&c.summary.id)?.is_some() {
            return Err(StorageError::Conflict);
        }
        checkpoint_validation::validate(c).map_err(|_| invalid())?;
        let blob = serde_json::to_vec(c).map_err(|_| invalid())?;
        let changed = self.connection.execute(
            "INSERT OR IGNORE INTO checkpoints(id,project_id,source_session_id,snapshot) SELECT ?1,?2,?3,?4 WHERE NOT EXISTS (SELECT 1 FROM deleted_checkpoints WHERE id=?1)",
            params![c.summary.id,c.summary.project_id,value.source_session_id,blob])?;
        if changed != 1 {
            return Err(StorageError::Conflict);
        }
        Ok(())
    }
    pub fn checkpoint(&self, id: &str) -> Result<Option<StoredCheckpoint>, StorageError> {
        let row: Option<(String, Vec<u8>)> = self
            .connection
            .query_row(
                "SELECT source_session_id,snapshot FROM checkpoints WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(source_session_id, bytes)| {
            let checkpoint: Checkpoint = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
            if checkpoint.summary.id != id {
                return Err(invalid());
            }
            checkpoint_validation::validate(&checkpoint).map_err(|_| invalid())?;
            Ok(StoredCheckpoint {
                source_session_id,
                checkpoint,
            })
        })
        .transpose()
    }
    pub fn list_checkpoints(
        &self,
        project_id: &str,
        after: Option<&str>,
        limit: u32,
    ) -> Result<Vec<CheckpointSummary>, StorageError> {
        if !(1..=51).contains(&limit) {
            return Err(invalid());
        }
        let mut statement = self.connection.prepare(
            "SELECT id FROM checkpoints WHERE project_id=?1 AND id>?2 ORDER BY id LIMIT ?3",
        )?;
        let ids = statement
            .query_map(params![project_id, after.unwrap_or(""), limit], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                self.checkpoint(&id)?
                    .map(|c| c.checkpoint.summary)
                    .ok_or_else(invalid)
            })
            .collect()
    }
}
