use crate::{JobKind, RecoveryStore, StorageError};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

/// Storage owns durability/CAS, while jobs/host validate serialized domain data.
/// Input is immutable and separate from mutable progress/result state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredJob {
    pub id: String,
    pub kind: JobKind,
    pub session_id: String,
    pub input: Vec<u8>,
    pub version: u32,
    pub state: Vec<u8>,
}
fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredJob> {
    Ok(StoredJob {
        id: row.get(0)?,
        kind: row.get(5)?,
        session_id: row.get(1)?,
        input: row.get(2)?,
        version: row.get(3)?,
        state: row.get(4)?,
    })
}
impl RecoveryStore {
    /// Replaying a request returns the latest state without replacing it with queued.
    /// A reused ID with different immutable input/session is a conflict.
    pub fn create_job(&mut self, job: &StoredJob) -> Result<StoredJob, StorageError> {
        if job.version != 0 {
            return Err(StorageError::InvalidVersion);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id,session_id,input,version,state,kind FROM jobs WHERE id=?1",
                [&job.id],
                row,
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing.kind != job.kind
                || existing.session_id != job.session_id
                || existing.input != job.input
            {
                return Err(StorageError::Conflict);
            }
            tx.commit()?;
            return Ok(existing);
        }
        let known: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            [&job.session_id],
            |r| r.get(0),
        )?;
        if !known {
            return Err(StorageError::Conflict);
        }
        tx.execute(
            "INSERT INTO jobs(id,session_id,input,version,state,kind) VALUES(?1,?2,?3,0,?4,?5)",
            params![job.id, job.session_id, job.input, job.state, job.kind],
        )?;
        tx.commit()?;
        Ok(job.clone())
    }
    pub fn load_job(&self, id: &str) -> Result<Option<StoredJob>, StorageError> {
        Ok(self
            .connection
            .query_row(
                "SELECT id,session_id,input,version,state,kind FROM jobs WHERE id=?1",
                [id],
                row,
            )
            .optional()?)
    }
    /// Progress, phase, errors and result references are committed as one state blob.
    pub fn update_job(
        &mut self,
        id: &str,
        kind: JobKind,
        expected_version: u32,
        state: &[u8],
    ) -> Result<u32, StorageError> {
        let next = expected_version
            .checked_add(1)
            .ok_or(StorageError::InvalidVersion)?;
        let changed = self.connection.execute(
            "UPDATE jobs SET version=?1,state=?2 WHERE id=?3 AND version=?4 AND kind=?5",
            params![next, state, id, expected_version, kind],
        )?;
        if changed != 1 {
            return Err(StorageError::Conflict);
        }
        Ok(next)
    }
    /// IDs only, so listing does not deserialize or copy all snapshot inputs.
    /// Stable UUID order with bounded pages. Caller repeats from None after restart.
    pub fn list_jobs(
        &self,
        kind: Option<JobKind>,
        session_id: Option<&str>,
        after: Option<&str>,
        limit: u32,
    ) -> Result<Vec<String>, StorageError> {
        if !(1..=100).contains(&limit) {
            return Err(StorageError::InvalidVersion);
        }
        let mut statement = self.connection.prepare("SELECT id FROM jobs WHERE (?1 IS NULL OR session_id=?1) AND (?2 IS NULL OR id>?2) AND (?4 IS NULL OR kind=?4) ORDER BY id LIMIT ?3")?;
        Ok(statement
            .query_map(params![session_id, after, limit, kind], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}
