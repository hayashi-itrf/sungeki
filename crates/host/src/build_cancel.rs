//! Persist the parent request first, then notify the owned child. Retrying after
//! a child-notification failure is safe; this never certifies resource cleanup.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_protocol::{ApiError, RenderJobRequest};
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn request(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<StoredJob, ApiError> {
    let (parent, child) = {
        let mut sessions = shared.lock().map_err(invalid)?;
        let (_, state) = build::decode(expected)?;
        let child = match &state.child_id {
            Some(id) => sessions.store.load_job(id).map_err(storage_error)?,
            None => None,
        };
        if let Some(child) = &child {
            let owner = match child.kind {
                JobKind::Speech => crate::speech_journal::decode_input(child)?
                    .0
                    .parent_job_id()
                    .map(str::to_owned),
                JobKind::Render => crate::render_record::decode(child)?.0.parent_job_id,
                JobKind::Review => crate::review_record::decode(child)?.0.parent_job_id,
                _ => return Err(invalid("制作子の種類が不正です")),
            };
            if child.session_id != expected.session_id
                || owner.as_deref() != Some(expected.id.as_str())
            {
                return Err(invalid("取消先の所有が一致しません"));
            }
        }
        let parent = build::request_cancel(&mut sessions, expected)?;
        if build::decode(&parent)?.1.phase != sungeki_project::JobPhase::Cancelling {
            return Ok(parent);
        }
        (parent, child)
    };
    if let Some(child) = child {
        match child.kind {
            JobKind::Speech => {
                crate::speech_runtime::cancel(shared, &child.id)?;
            }
            JobKind::Render => {
                crate::render_api::execute(shared, RenderJobRequest::Cancel { job_id: child.id })?;
            }
            JobKind::Review => {
                crate::review_jobs::cancel(shared, child.id)?;
            }
            _ => unreachable!(),
        }
    }
    Ok(parent)
}
