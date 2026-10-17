use crate::{
    sessions::{Sessions, storage_error},
    speech_journal::{decode_input as decode, save},
    speech_state::{Execution, invalid},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::StoredJob;

pub(crate) fn load(sessions: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音声ジョブが見つかりません"))
}
/// Preserve durable cancellation precedence. Unknown external execution can be
/// Interrupted, never Cancelled. This method does not settle resource leases.
pub(crate) fn record(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    error: ApiError,
    cancelled: &AtomicBool,
) -> Result<(), ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let mut stored = load(&sessions, id)?;
    let (_, mut state) = decode(&stored)?;
    if state.phase.is_terminal() {
        return Ok(());
    }
    let cancel = state.phase == JobPhase::Cancelling
        || error.code == ErrorCode::Cancelled
        || cancelled.load(Ordering::Acquire);
    if cancel && state.phase != JobPhase::Cancelling {
        state.phase = JobPhase::Cancelling;
        save(&mut sessions, &stored, &state)?;
        stored = load(&sessions, id)?;
    } else if state.phase == JobPhase::Queued {
        // The preparation stage failed without sending a synthesis request.
        state.phase = JobPhase::Running;
        save(&mut sessions, &stored, &state)?;
        stored = load(&sessions, id)?;
    }
    state.phase = if cancel {
        if state.execution == Execution::PossiblyRunning {
            JobPhase::Interrupted
        } else {
            JobPhase::Cancelled
        }
    } else {
        JobPhase::Failed
    };
    state.error = Some(error);
    save(&mut sessions, &stored, &state)
}
