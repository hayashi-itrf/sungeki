use crate::{RecoveryStore, StorageError, StoredJob, StoredSession};
use rusqlite::{TransactionBehavior, params};

impl RecoveryStore {
    /// Atomically persist the editor snapshot (including Undo and operation
    /// receipt) and its job's application result. Domain validation belongs to
    /// the host. A conflict or either write failure rolls back both records.
    pub fn commit_job_application(
        &mut self,
        expected_session_version: u32,
        session: &StoredSession,
        expected_job: &StoredJob,
        job_state: &[u8],
    ) -> Result<u32, StorageError> {
        Ok(self.commit_applications(
            expected_session_version,
            session,
            &[(expected_job, job_state)],
        )?[0])
    }

    /// Commit a production parent's receipt, its speech child's application and
    /// the editor snapshot together. Both jobs must belong to this session.
    pub fn commit_build_speech_application(
        &mut self,
        expected_session_version: u32,
        session: &StoredSession,
        parent: &StoredJob,
        parent_state: &[u8],
        child: &StoredJob,
        child_state: &[u8],
    ) -> Result<(u32, u32), StorageError> {
        if parent.kind != crate::JobKind::BuildVideo
            || child.kind != crate::JobKind::Speech
            || parent.id == child.id
        {
            return Err(StorageError::Conflict);
        }
        let versions = self.commit_applications(
            expected_session_version,
            session,
            &[(parent, parent_state), (child, child_state)],
        )?;
        Ok((versions[0], versions[1]))
    }

    fn commit_applications(
        &mut self,
        expected_session_version: u32,
        session: &StoredSession,
        jobs: &[(&StoredJob, &[u8])],
    ) -> Result<Vec<u32>, StorageError> {
        if jobs.iter().any(|(job, _)| session.id != job.session_id) {
            return Err(StorageError::Conflict);
        }
        if expected_session_version.checked_add(1) != Some(session.version) {
            return Err(StorageError::InvalidVersion);
        }
        let versions = jobs
            .iter()
            .map(|(job, _)| {
                job.version
                    .checked_add(1)
                    .ok_or(StorageError::InvalidVersion)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::recovery::write_session(&tx, Some(expected_session_version), session)?;
        for ((job, state), version) in jobs.iter().zip(&versions) {
            let changed = tx.execute(
                "UPDATE jobs SET version=?1,state=?2 WHERE id=?3 AND version=?4 AND kind=?5 AND session_id=?6 AND input=?7",
                params![version, state, job.id, job.version, job.kind, session.id, job.input],
            )?;
            if changed != 1 {
                return Err(StorageError::Conflict);
            }
        }
        tx.commit()?;
        Ok(versions)
    }
}
