use crate::{JobKind, RecoveryStore, StorageError, StoredJob, StoredSession};
use rusqlite::{TransactionBehavior, params};

impl RecoveryStore {
    /// Create the new production session and immutable parent plan together.
    /// Replay is resolved by the host before calling this new-only operation.
    pub fn create_build(
        &mut self,
        session: &StoredSession,
        parent: &StoredJob,
    ) -> Result<(), StorageError> {
        if parent.kind != JobKind::BuildVideo || parent.session_id != session.id {
            return Err(StorageError::Conflict);
        }
        if session.version != 0 || session.revision != 0 || parent.version != 0 {
            return Err(StorageError::InvalidVersion);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::recovery::write_session(&tx, None, session)?;
        tx.execute(
            "INSERT INTO jobs(id,session_id,input,version,state,kind) VALUES(?1,?2,?3,0,?4,?5)",
            params![
                parent.id,
                parent.session_id,
                parent.input,
                parent.state,
                parent.kind
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}
