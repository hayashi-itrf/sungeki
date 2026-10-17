use crate::{DocumentBinding, RecoveryStore, StorageError, StoredSession};
use rusqlite::params;

impl RecoveryStore {
    /// Register a loaded document and its recovery state in a single transaction.
    pub fn create_document_session(
        &mut self,
        state: &StoredSession,
        binding: &DocumentBinding,
    ) -> Result<(), StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        write_document_session(&tx, state, binding)?;
        tx.commit()?;
        Ok(())
    }

    /// New document registration and checkpoint import either both commit or neither does.
    pub fn create_document_session_with_checkpoints(
        &mut self,
        state: &StoredSession,
        binding: &DocumentBinding,
        project_id: &str,
        batch: &[crate::PreparedCheckpointImport],
    ) -> Result<crate::CheckpointImportOutcome, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        write_document_session(&tx, state, binding)?;
        let result = crate::checkpoint_import::write_imports(&tx, project_id, &state.id, batch)?;
        tx.commit()?;
        Ok(result)
    }
}

fn write_document_session(
    tx: &rusqlite::Transaction<'_>,
    state: &StoredSession,
    binding: &DocumentBinding,
) -> Result<(), StorageError> {
    if state.id != binding.session_id || state.revision != 0 || state.version != 0 {
        return Err(StorageError::InvalidVersion);
    }
    let taken: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM documents WHERE path=?1 OR (device=?2 AND inode=?3)) OR EXISTS(SELECT 1 FROM save_intents WHERE path=?1)",
        params![binding.path, binding.stamp.device.to_string(), binding.stamp.inode.to_string()], |r| r.get(0))?;
    if taken {
        return Err(StorageError::Conflict);
    }
    crate::recovery::write_session(tx, None, state)?;
    tx.execute("INSERT INTO documents(session_id,path,device,inode,file_hash,content_hash) VALUES(?1,?2,?3,?4,?5,?6)",
        params![binding.session_id,binding.path,binding.stamp.device.to_string(),binding.stamp.inode.to_string(),binding.stamp.hash,binding.saved_content_hash])?;
    Ok(())
}
