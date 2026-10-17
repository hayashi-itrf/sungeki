//! Project-independent durable preset records. The host validates typed payloads;
//! storage owns atomic compare-and-swap and retains deletion revisions.
use crate::StorageError;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPreset {
    pub id: String,
    pub revision: u32,
    /// None is a tombstone, not an unknown identifier.
    pub payload: Option<Vec<u8>>,
}

pub struct PresetLibrary {
    connection: Connection,
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPreset> {
    Ok(StoredPreset {
        id: row.get(0)?,
        revision: row.get(1)?,
        payload: row.get(2)?,
    })
}

impl PresetLibrary {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 1 {
            return Err(StorageError::UnsupportedVersion);
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS presets (
                id TEXT PRIMARY KEY NOT NULL,
                revision INTEGER NOT NULL CHECK(revision > 0),
                payload BLOB
            ); PRAGMA user_version=1;",
        )?;
        tx.commit()?;
        Ok(Self { connection })
    }

    /// Includes tombstones so clients can distinguish deleted and unknown IDs.
    pub fn get(&self, id: &str) -> Result<Option<StoredPreset>, StorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT id,revision,payload FROM presets WHERE id=?1",
                [id],
                row,
            )
            .optional()?)
    }

    /// Stable ID pagination; deleted records are never presented as usable presets.
    pub fn list(&self, after_id: &str, limit: u32) -> Result<Vec<StoredPreset>, StorageError> {
        let mut query = self.connection.prepare(
            "SELECT id,revision,payload FROM presets
             WHERE id>?1 AND payload IS NOT NULL ORDER BY id LIMIT ?2",
        )?;
        Ok(query
            .query_map(params![after_id, limit.min(100)], row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Revision zero creates a never-used ID. Updating/deleting requires the
    /// observed revision. Exact retries return the committed result. A deleted
    /// ID cannot be recreated: restoring a preset must allocate a new ID.
    pub fn write(
        &mut self,
        id: &str,
        expected_revision: u32,
        payload: Option<&[u8]>,
    ) -> Result<StoredPreset, StorageError> {
        if id.is_empty()
            || id.len() > 128
            || id.chars().any(char::is_control)
            || payload.is_some_and(|p| p.is_empty() || p.len() > 4 * 1024 * 1024)
        {
            return Err(StorageError::InvalidSnapshot);
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(StorageError::InvalidVersion)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id,revision,payload FROM presets WHERE id=?1",
                [id],
                row,
            )
            .optional()?;
        if let Some(existing) = &existing {
            if existing.revision == revision && existing.payload.as_deref() == payload {
                tx.commit()?;
                return Ok(existing.clone());
            }
            if existing.revision != expected_revision || existing.payload.is_none() {
                return Err(StorageError::Conflict);
            }
        } else if expected_revision != 0 || payload.is_none() {
            return Err(StorageError::Conflict);
        }
        tx.execute(
            "INSERT INTO presets(id,revision,payload) VALUES(?1,?2,?3)
             ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,payload=excluded.payload",
            params![id, revision, payload],
        )?;
        tx.commit()?;
        Ok(StoredPreset {
            id: id.into(),
            revision,
            payload: payload.map(<[u8]>::to_vec),
        })
    }
}
