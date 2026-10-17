use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{path::Path, time::Duration};

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    SnapshotEncoding(std::io::Error),
    InvalidSnapshot,
    Conflict,
    InvalidVersion,
    UnsupportedVersion,
}
impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct StoredSession {
    pub id: String,
    pub revision: u32,
    pub version: u32,
    // Shared immutable bytes avoid copying the full Undo journal on each cache hit.
    pub snapshot: std::sync::Arc<Vec<u8>>,
}

pub struct RecoveryStore {
    pub(crate) connection: Connection,
    snapshot_cache: std::cell::RefCell<crate::snapshot_cache::SnapshotCache>,
}
impl RecoveryStore {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: u32 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 18 {
            return Err(StorageError::UnsupportedVersion);
        }
        transaction.execute_batch("CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY NOT NULL, revision INTEGER NOT NULL CHECK(revision>=0), version INTEGER NOT NULL CHECK(version>=0), snapshot BLOB NOT NULL
        );
        CREATE TABLE IF NOT EXISTS documents (
            session_id TEXT PRIMARY KEY NOT NULL REFERENCES sessions(id),
            path TEXT UNIQUE NOT NULL, device TEXT NOT NULL, inode TEXT NOT NULL,
            file_hash TEXT NOT NULL, content_hash TEXT NOT NULL, UNIQUE(device,inode)
        ); CREATE TABLE IF NOT EXISTS save_intents (
            session_id TEXT PRIMARY KEY NOT NULL, path TEXT UNIQUE NOT NULL, content_hash TEXT NOT NULL
        ); CREATE TABLE IF NOT EXISTS checkpoints (
            id TEXT PRIMARY KEY NOT NULL, project_id TEXT NOT NULL, source_session_id TEXT NOT NULL, snapshot BLOB NOT NULL
        ); CREATE INDEX IF NOT EXISTS checkpoints_project ON checkpoints(project_id,id); CREATE TABLE IF NOT EXISTS deleted_checkpoints (id TEXT PRIMARY KEY NOT NULL, project_id TEXT NOT NULL); CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
            input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL
        ); CREATE INDEX IF NOT EXISTS jobs_session ON jobs(session_id,id);")?;
        if version < 7 {
            // Sungeki's internal recovery schema v6 contained review jobs only.
            // Upgrade existing Sungeki records once; never decode another job kind as review.
            transaction.execute_batch("ALTER TABLE jobs ADD COLUMN kind TEXT NOT NULL DEFAULT 'review' CHECK(kind IN ('review','speech')); PRAGMA user_version=7;")?;
        }
        if version < 8 {
            transaction.execute_batch("CREATE TABLE jobs_v8 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render'))
            ); INSERT INTO jobs_v8(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v8 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=8;")?;
        }
        if version < 9 {
            transaction.execute_batch("CREATE TABLE jobs_v9 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness'))
            ); INSERT INTO jobs_v9(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v9 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=9;")?;
        }
        if version < 10 {
            transaction.execute_batch("CREATE TABLE jobs_v10 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview'))
            ); INSERT INTO jobs_v10(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v10 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=10;")?;
        }
        if version < 11 {
            transaction.execute_batch("CREATE TABLE jobs_v11 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview','build_video'))
            ); INSERT INTO jobs_v11(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v11 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=11;")?;
        }
        if version < 12 {
            transaction.execute_batch("CREATE TABLE jobs_v12 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview','build_video','frame_preview'))
            ); INSERT INTO jobs_v12(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v12 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=12;")?;
        }
        if version < 13 {
            transaction.execute_batch("CREATE TABLE jobs_v13 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview','build_video','frame_preview','collection'))
            ); INSERT INTO jobs_v13(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v13 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=13;")?;
        }
        if version < 14 {
            // Preserve earlier Sungeki collection histories while adding typed errors.
            transaction.execute_batch("UPDATE jobs SET state=CAST(json_set(CAST(state AS TEXT), '$.error',
                json_object('code', CASE WHEN json_extract(CAST(state AS TEXT), '$.phase')='cancelled'
                    THEN 'CANCELLED' ELSE 'IO_ERROR' END,
                    'message', json_extract(CAST(state AS TEXT), '$.error'),
                    'details', json('{}'), 'retryable', json('false'), 'suggested_actions', json('[]'))) AS BLOB)
                WHERE kind='collection' AND json_valid(CAST(state AS TEXT))
                    AND json_type(CAST(state AS TEXT), '$.error')='text';
                PRAGMA user_version=14;")?;
        }
        if version < 15 {
            transaction.execute_batch(
                "CREATE TABLE IF NOT EXISTS checkpoint_imports (
                id TEXT PRIMARY KEY NOT NULL, portable BLOB NOT NULL, runtime_hash TEXT NOT NULL
            ); PRAGMA user_version=15;",
            )?;
        }
        if version < 16 {
            transaction.execute_batch("CREATE TABLE jobs_v16 (
                id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL REFERENCES sessions(id),
                input BLOB NOT NULL, version INTEGER NOT NULL CHECK(version>=0), state BLOB NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('review','speech','render','loudness','audio_preview','build_video','frame_preview','collection','proxy'))
            ); INSERT INTO jobs_v16(id,session_id,input,version,state,kind)
                SELECT id,session_id,input,version,state,kind FROM jobs;
            DROP TABLE jobs; ALTER TABLE jobs_v16 RENAME TO jobs;
            CREATE INDEX jobs_session ON jobs(session_id,id); PRAGMA user_version=16;")?;
        }
        transaction.execute_batch(
            "CREATE INDEX IF NOT EXISTS jobs_kind_session ON jobs(kind,session_id,id);",
        )?;
        if version < 18 {
            crate::snapshot_codec::migrate(&transaction, version == 17)?;
            transaction.pragma_update(None, "user_version", 18)?;
        }
        transaction.commit()?;
        Ok(Self {
            connection,
            snapshot_cache: Default::default(),
        })
    }
    pub fn load(&self, id: &str) -> Result<Option<StoredSession>, StorageError> {
        let row: Option<(String, u32, u32, Vec<u8>)> = self
            .connection
            .query_row(
                "SELECT id,revision,version,snapshot FROM sessions WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        row.map(|(id, revision, version, encoded)| {
            Ok(StoredSession {
                id,
                revision,
                version,
                snapshot: self.snapshot_cache.borrow_mut().decode(encoded)?,
            })
        })
        .transpose()
    }
    pub fn list(&self) -> Result<Vec<String>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM sessions ORDER BY id")?;
        Ok(statement
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
    // Snapshot includes the document, bounded history and unbounded successful
    // operation receipts. Host replaces its in-memory state only after commit.
    pub fn commit(
        &mut self,
        expected: Option<u32>,
        state: &StoredSession,
    ) -> Result<(), StorageError> {
        if expected.is_none() && (state.revision != 0 || state.version != 0) {
            return Err(StorageError::InvalidVersion);
        }
        if let Some(previous) = expected {
            if Some(state.version) != previous.checked_add(1) {
                return Err(StorageError::InvalidVersion);
            }
        }
        // Compression depends only on this immutable candidate, not on database
        // state. Keep it outside the writer lock; the SQL version predicate below
        // still rejects a candidate if another connection commits meanwhile.
        let encoded = crate::snapshot_codec::encode(&state.snapshot)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        write_encoded_session(&transaction, expected, state, &encoded)?;
        transaction.commit()?;
        self.snapshot_cache
            .get_mut()
            .remember(encoded, &state.snapshot);
        Ok(())
    }
}

pub(crate) fn write_session(
    transaction: &rusqlite::Transaction<'_>,
    expected: Option<u32>,
    state: &StoredSession,
) -> Result<Vec<u8>, StorageError> {
    let snapshot = crate::snapshot_codec::encode(&state.snapshot)?;
    write_encoded_session(transaction, expected, state, &snapshot)?;
    Ok(snapshot)
}

fn write_encoded_session(
    transaction: &rusqlite::Transaction<'_>,
    expected: Option<u32>,
    state: &StoredSession,
    snapshot: &[u8],
) -> Result<(), StorageError> {
    let changed = match expected {
        None => transaction.execute(
            "INSERT OR IGNORE INTO sessions(id,revision,version,snapshot) VALUES(?1,?2,?3,?4)",
            params![state.id, state.revision, state.version, snapshot],
        )?,
        Some(revision) => transaction.execute(
            "UPDATE sessions SET revision=?2,version=?3,snapshot=?4 WHERE id=?1 AND version=?5",
            params![state.id, state.revision, state.version, snapshot, revision],
        )?,
    };
    if changed != 1 {
        return Err(StorageError::Conflict);
    }
    Ok(())
}
