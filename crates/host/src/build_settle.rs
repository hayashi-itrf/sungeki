//! Read-only settlement evidence before the parent cancellation commit.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn settle(sessions: &mut Sessions, expected: &StoredJob) -> Result<bool, ApiError> {
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
    {
        return Err(invalid("制作状態が変更されています"));
    }
    let (input, mut state) = build::decode(expected)?;
    if state.phase == JobPhase::Cancelled {
        return Ok(true);
    }
    if state.phase != JobPhase::Cancelling {
        return Err(invalid("取消要求がありません"));
    }
    if !super::settlement::ready(sessions, expected)? {
        return Ok(false);
    }
    state.phase = JobPhase::Cancelled;
    state.child_id = None;
    state.validate(&input)?;
    sessions
        .store
        .update_job(
            &expected.id,
            JobKind::BuildVideo,
            expected.version,
            &serde_json::to_vec(&state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    sessions.build_runtime.notify_terminal();
    Ok(true)
}
