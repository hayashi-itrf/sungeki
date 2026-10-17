//! Shared read-only settlement check for cancellation and explicit continuation.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use sungeki_jobs::{BuildSchedule, BuildStep};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn ready(sessions: &Sessions, expected: &StoredJob) -> Result<bool, ApiError> {
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
    {
        return Err(invalid("制作状態が変更されています"));
    }
    let (input, state) = build::decode(expected)?;
    if sessions.build_runtime.owns(&expected.id)? {
        return Ok(false);
    }
    let schedule = BuildSchedule::restore(
        input.speech_count,
        state.receipts.len() as u16,
        state.child_id.clone(),
    )
    .map_err(invalid)?;
    for id in &input.step_ids {
        if sessions.speech_runtime.owns(id)?
            || sessions.speech_resources.has_owner(id).map_err(invalid)?
            || sessions
                .render_runtime
                .flags
                .lock()
                .map_err(invalid)?
                .contains_key(id)
            || sessions
                .review_runtime
                .cancellations
                .lock()
                .map_err(invalid)?
                .contains_key(id)
        {
            return Ok(false);
        }
        let Some(child) = sessions.store.load_job(id).map_err(storage_error)? else {
            if state.child_id.as_ref() == Some(id)
                && matches!(
                    schedule.step(),
                    Some(BuildStep::Speech(_) | BuildStep::Render | BuildStep::Review)
                )
            {
                return Ok(false); // Lost dispatch outcome is not evidence of non-execution.
            }
            continue;
        };
        let (owner, phase, uncertain) = match child.kind {
            JobKind::Speech => {
                let (i, s) = crate::speech_journal::decode_input(&child)?;
                (
                    i.parent_job_id().map(str::to_owned),
                    s.phase,
                    s.execution == crate::speech_state::Execution::PossiblyRunning,
                )
            }
            JobKind::Render => {
                let (i, s) = crate::render_record::decode(&child)?;
                (i.parent_job_id, s.phase, false)
            }
            JobKind::Review => {
                let (i, s) = crate::review_record::decode(&child)?;
                let root = sessions.review_directory.as_ref().map_err(Clone::clone)?;
                let path = crate::review_workdir::path(root, id)?;
                let remains = match std::fs::symlink_metadata(path) {
                    Ok(_) => true,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                    Err(e) => return Err(invalid(e)),
                };
                (i.parent_job_id, s.phase, remains)
            }
            _ => return Err(invalid("制作工程IDが別種類のジョブに使用されています")),
        };
        if owner.as_deref() != Some(expected.id.as_str()) || child.session_id != expected.session_id
        {
            return Err(invalid("終了確認対象の子所属が一致しません"));
        }
        if uncertain
            || !matches!(
                phase,
                JobPhase::Succeeded | JobPhase::Failed | JobPhase::Cancelled
            )
        {
            return Ok(false);
        }
    }
    Ok(true)
}
