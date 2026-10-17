use crate::{FileStamp, RecoveryStore, StorageError};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentBinding {
    pub session_id: String,
    pub path: String,
    pub stamp: FileStamp,
    pub saved_content_hash: String,
}
impl RecoveryStore {
    pub fn document_binding(
        &self,
        session_id: &str,
    ) -> Result<Option<DocumentBinding>, StorageError> {
        self.connection.query_row("SELECT path,device,inode,file_hash,content_hash FROM documents WHERE session_id=?1", [session_id], |row| {
            Ok(DocumentBinding { session_id: session_id.into(), path: row.get(0)?,
                stamp: FileStamp { device: row.get::<_, String>(1)?.parse().map_err(|_| rusqlite::Error::InvalidQuery)?, inode: row.get::<_, String>(2)?.parse().map_err(|_| rusqlite::Error::InvalidQuery)?, hash: row.get(3)? },
                saved_content_hash: row.get(4)? })
        }).optional().map_err(Into::into)
    }
    pub fn document_owner(
        &self,
        path: &str,
        stamp: Option<&FileStamp>,
    ) -> Result<Option<String>, StorageError> {
        let identity = stamp.map(|s| (s.device.to_string(), s.inode.to_string()));
        self.connection.query_row("SELECT session_id FROM documents WHERE path=?1 OR (device=?2 AND inode=?3) LIMIT 1",
            params![path, identity.as_ref().map(|v| &v.0), identity.as_ref().map(|v| &v.1)], |row| row.get(0)).optional().map_err(Into::into)
    }
    // Does not change document revision/history: saving and editing are separate.
    // The host must own a destination lease before file I/O and update this record only after successful publication.
    pub fn bind_document(&mut self, binding: &DocumentBinding) -> Result<(), StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            [&binding.session_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(StorageError::Conflict);
        }
        let owner: Option<String> = tx.query_row("SELECT session_id FROM documents WHERE (path=?1 OR (device=?2 AND inode=?3)) AND session_id<>?4 LIMIT 1",
            params![binding.path, binding.stamp.device.to_string(), binding.stamp.inode.to_string(), binding.session_id], |r|r.get(0)).optional()?;
        if owner.is_some() {
            return Err(StorageError::Conflict);
        }
        tx.execute("INSERT INTO documents(session_id,path,device,inode,file_hash,content_hash) VALUES(?1,?2,?3,?4,?5,?6)
            ON CONFLICT(session_id) DO UPDATE SET path=excluded.path,device=excluded.device,inode=excluded.inode,file_hash=excluded.file_hash,content_hash=excluded.content_hash",
            params![binding.session_id,binding.path,binding.stamp.device.to_string(),binding.stamp.inode.to_string(),binding.stamp.hash,binding.saved_content_hash])?;
        tx.execute(
            "DELETE FROM save_intents WHERE session_id=?1",
            [&binding.session_id],
        )?;
        tx.commit()?;
        Ok(())
    }
}
