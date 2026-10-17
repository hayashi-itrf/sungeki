use crate::{
    build_record::decode,
    build_state::{Receipt, invalid},
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

/// Blocking preparation; no edits, synthesis, rendering or file publication.
pub(crate) fn prepare(shared: &Arc<Mutex<Sessions>>, expected: &StoredJob) -> Result<(), ApiError> {
    let (input, mut state, version) = {
        let sessions = shared.lock().map_err(invalid)?;
        check(&sessions, expected)?;
        let (input, state) = decode(expected)?;
        if !state.receipts.is_empty() {
            return Ok(());
        }
        let (stored, current) = sessions.load(&expected.session_id)?;
        if state.phase != JobPhase::Running
            || state.child_id.as_ref() != Some(&input.step_ids[0])
            || stored.revision != 0
            || current.history().project().id != input.plan.project_id
            || !current.history().project().clips.is_empty()
        {
            return Err(invalid("準備工程の予約または新規作品が一致しません"));
        }
        (input, state, stored.version)
    };
    super::presets::validate(&input.recipe)?;
    crate::build_record::verify_sources(shared, &input.recipe, || {
        shared.lock().map_or(true, |s| check(&s, expected).is_err())
    })?;
    let mut sessions = shared.lock().map_err(invalid)?;
    check(&sessions, expected)?;
    if sessions.load(&expected.session_id)?.0.version != version {
        return Err(invalid("素材確認中に作品が変更されました"));
    }
    state.receipts.push(Receipt {
        id: input.step_ids[0].clone(),
        revision: 0,
    });
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
    Ok(())
}
fn check(sessions: &Sessions, expected: &StoredJob) -> Result<(), ApiError> {
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
    {
        return Err(invalid("制作状態が変更されています"));
    }
    Ok(())
}
