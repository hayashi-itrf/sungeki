use crate::{RecoveryStore, StorageError};
use rusqlite::params;
#[derive(Debug)]
pub struct SaveIntent {
    pub session_id: String,
    pub path: String,
    pub content_hash: String,
}
impl RecoveryStore {
    pub fn begin_save(&mut self, intent: &SaveIntent) -> Result<(), StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let taken: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM documents WHERE path=?1 AND session_id<>?2) OR EXISTS(SELECT 1 FROM save_intents WHERE path=?1 OR session_id=?2)",params![intent.path,intent.session_id],|r|r.get(0))?;
        if taken {
            return Err(StorageError::Conflict);
        }
        tx.execute(
            "INSERT INTO save_intents(session_id,path,content_hash) VALUES(?1,?2,?3)",
            params![intent.session_id, intent.path, intent.content_hash],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn save_intents(&self) -> Result<Vec<SaveIntent>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT session_id,path,content_hash FROM save_intents ORDER BY session_id")?;
        Ok(statement
            .query_map([], |r| {
                Ok(SaveIntent {
                    session_id: r.get(0)?,
                    path: r.get(1)?,
                    content_hash: r.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn abandon_save(&mut self, session_id: &str) -> Result<(), StorageError> {
        self.connection
            .execute("DELETE FROM save_intents WHERE session_id=?1", [session_id])?;
        Ok(())
    }
}
