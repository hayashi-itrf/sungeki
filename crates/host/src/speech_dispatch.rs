//! Dispatch ownership joins the durable journal and in-memory resource lease.
//! The worker must verify engine identity while reserved, dispatch only after
//! `begin` returns, and confirm completion only with evidence of remote settlement.
use crate::{
    sessions::{Sessions, storage_error},
    speech_journal::{Input, decode_input as decode, save},
    speech_state::{Execution, invalid},
};
use sungeki_jobs::{ResourceError, ResourceLease};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};

use sungeki_storage::StoredJob;

pub(crate) struct Reservation {
    job_id: String,
    lease: ResourceLease,
    input: Input,
}
pub(crate) struct Dispatched {
    job_id: String,
    lease: ResourceLease,
}
impl Reservation {
    pub fn acquire(sessions: &mut Sessions, job_id: &str) -> Result<Self, ApiError> {
        let record = load(sessions, job_id)?;
        let (input, state) = decode(&record)?;
        if state.phase != JobPhase::Queued {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "待機中の音声ジョブではありません",
            ));
        }
        let lease = sessions
            .speech_resources
            .try_acquire(job_id, input.resources())
            .map_err(resource_error)?;
        Ok(Self {
            job_id: job_id.into(),
            lease,
            input,
        })
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    /// Dropping before begin releases the reservation without dispatching work.
    /// A cancellation may arrive during asynchronous engine identity checks.
    pub fn begin(mut self, sessions: &mut Sessions) -> Result<Dispatched, ApiError> {
        let record = load(sessions, &self.job_id)?;
        let (_, mut state) = decode(&record)?;
        if state.phase != JobPhase::Queued {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "送信前に音声ジョブが変更されました",
            ));
        }
        self.input
            .validate_parent(sessions, &record.id, &record.session_id)?;
        // Start the in-memory guard FIRST: even an ambiguous DB commit failure
        // must retain ownership. Only a successful durable commit permits HTTP.
        self.lease.start().map_err(resource_error)?;
        state.phase = JobPhase::Running;
        state.execution = Execution::PossiblyRunning;
        save(sessions, &record, &state)?;
        Ok(Dispatched {
            job_id: self.job_id,
            lease: self.lease,
        })
    }
}
impl Dispatched {
    /// This is NOT for timeouts, transport failures or a successful health check.
    /// Caller has drained the remote response or proved the owned process ended.
    /// Preserve current cancellation/error state; finishing execution is separate
    /// from verifying/storing WAV and applying the result to Project.
    pub fn confirm_finished(self, sessions: &mut Sessions) -> Result<(), ApiError> {
        let record = load(sessions, &self.job_id)?;
        let (_, mut state) = decode(&record)?;
        if state.execution != Execution::PossiblyRunning {
            return Err(invalid("終了確認対象の外部実行状態が不正です"));
        }
        state.execution = Execution::Finished;
        save(sessions, &record, &state)?;
        // Never release first: crash recovery must observe Finished durably.
        self.lease.confirm_finished().map_err(resource_error)
    }
}
fn load(sessions: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音声ジョブが見つかりません"))
}
fn resource_error(error: ResourceError) -> ApiError {
    let code = match error {
        ResourceError::Busy | ResourceError::Quarantined => ErrorCode::Conflict,
        _ => ErrorCode::IoError,
    };
    ApiError::new(code, format!("音声実行枠: {error:?}"))
}
