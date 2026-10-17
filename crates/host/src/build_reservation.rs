//! Persist reservations before dispatch. An existing reservation is a recovery
//! lookup, never authority to start the child process for a second time.
use crate::{
    build_record::decode,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use sungeki_jobs::BuildSchedule;
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

#[derive(Debug, PartialEq)]
pub(crate) enum Reservation {
    New(StoredJob),
    Existing(StoredJob),
}
fn current(sessions: &Sessions, expected: &StoredJob) -> Result<(), ApiError> {
    let stored = sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("親記録がありません"))?;
    if stored != *expected {
        return Err(invalid("親記録が更新済みです"));
    }
    Ok(())
}
pub(crate) fn reserve(
    sessions: &mut Sessions,
    expected: &StoredJob,
) -> Result<Reservation, ApiError> {
    current(sessions, expected)?;
    let (input, mut state) = decode(expected)?;
    if !matches!(state.phase, JobPhase::Queued | JobPhase::Running) {
        return Err(invalid("実行可能な親ジョブではありません"));
    }
    let (stored, candidate) = sessions.load(&expected.session_id)?;
    if stored.revision != state.expected_revision
        || candidate.history().project().id != input.plan.project_id
    {
        return Err(invalid("制作対象またはrevisionが変わりました"));
    }
    if state.child_id.is_some() {
        return Ok(Reservation::Existing(expected.clone()));
    }
    let mut schedule =
        BuildSchedule::restore(input.speech_count, state.receipts.len() as u16, None)
            .map_err(invalid)?;
    let id = input
        .step_ids
        .get(state.receipts.len())
        .ok_or_else(|| invalid("完了済みです"))?;
    schedule.reserve(id).map_err(invalid)?;
    state.phase = JobPhase::Running;
    state.child_id = schedule.child_id().map(str::to_owned);
    state.validate(&input)?;
    let bytes = serde_json::to_vec(&state).map_err(invalid)?;
    let version = sessions
        .store
        .update_job(&expected.id, JobKind::BuildVideo, expected.version, &bytes)
        .map_err(storage_error)?;
    Ok(Reservation::New(StoredJob {
        version,
        state: bytes,
        ..expected.clone()
    }))
}

/// Request only. The dispatcher must settle the actual child and resources
/// before acknowledging cancellation; this never claims cleanup is complete.
pub(crate) fn request_cancel(
    sessions: &mut Sessions,
    expected: &StoredJob,
) -> Result<StoredJob, ApiError> {
    current(sessions, expected)?;
    let (input, mut state) = decode(expected)?;
    if state.phase.is_terminal() || state.phase == JobPhase::Cancelling {
        return Ok(expected.clone());
    }
    let child_flag = if let Some(child_id) = &state.child_id {
        match sessions.store.load_job(child_id).map_err(storage_error)? {
            Some(child) if child.kind == JobKind::Render => {
                let (input, _) = crate::render_record::decode(&child)?;
                if input.parent_job_id.as_deref() != Some(expected.id.as_str()) {
                    return Err(invalid("出力子の所属が一致しません"));
                }
                sessions
                    .render_runtime
                    .flags
                    .lock()
                    .map_err(invalid)?
                    .get(child_id)
                    .cloned()
            }
            Some(child) if child.kind == JobKind::Review => {
                let (input, _) = crate::review_record::decode(&child)?;
                if input.parent_job_id.as_deref() != Some(expected.id.as_str()) {
                    return Err(invalid("確認子の所属が一致しません"));
                }
                sessions
                    .review_runtime
                    .cancellations
                    .lock()
                    .map_err(invalid)?
                    .get(child_id)
                    .cloned()
            }
            _ => None,
        }
    } else {
        None
    };
    state.phase = JobPhase::Cancelling;
    state.validate(&input)?;
    let bytes = serde_json::to_vec(&state).map_err(invalid)?;
    let version = sessions
        .store
        .update_job(&expected.id, JobKind::BuildVideo, expected.version, &bytes)
        .map_err(storage_error)?;
    if let Some(flag) = child_flag {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    Ok(StoredJob {
        version,
        state: bytes,
        ..expected.clone()
    })
}
