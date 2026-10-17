use crate::{
    build_record::decode,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse, StartRenderArgs};
use sungeki_storage::StoredJob;

pub(crate) fn validate_parent(
    sessions: &Sessions,
    parent_id: &str,
    child_id: &str,
    session_id: &str,
    revision: u32,
) -> Result<String, ApiError> {
    validate_step(sessions, parent_id, child_id, session_id, revision, 6)
}
pub(crate) fn validate_step(
    sessions: &Sessions,
    parent_id: &str,
    child_id: &str,
    session_id: &str,
    revision: u32,
    offset: usize,
) -> Result<String, ApiError> {
    let parent = sessions
        .store
        .load_job(parent_id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作親がありません"))?;
    let (input, state) = decode(&parent)?;
    if matches!(state.phase, JobPhase::Cancelling | JobPhase::Cancelled) {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "制作親が取消要求を受けています",
        ));
    }
    let (stored, current) = sessions.load(session_id)?;
    let saved = state
        .saved
        .as_ref()
        .ok_or_else(|| invalid("保存済み作品がありません"))?;
    let index = usize::from(input.speech_count) + offset;
    let hash = sungeki_storage::project_hash(current.history().project()).map_err(invalid)?;
    if parent.session_id != session_id
        || state.phase != JobPhase::Running
        || state.receipts.len() != index
        || state.child_id.as_deref() != Some(child_id)
        || state.expected_revision != revision
        || stored.revision != revision
        || saved.revision != revision
        || saved.content_hash != hash
        || current.history().project().id != input.plan.project_id
    {
        return Err(invalid("出力予約と保存作品が一致しません"));
    }
    Ok(hash)
}
/// Blocking document verification; call from the parent's blocking dispatcher.
pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    parent: &StoredJob,
) -> Result<SessionResponse, ApiError> {
    crate::build_record::save_project(shared, parent)?; // Verify existing binding/file, never rewrite.
    let (_, state) = decode(parent)?;
    let args = StartRenderArgs {
        wait_ms: None,
        session_id: parent.session_id.clone(),
        job_id: state
            .child_id
            .ok_or_else(|| invalid("出力工程が未予約です"))?,
        expected_revision: state.expected_revision,
    };
    crate::render_api::start_bound(shared, args, Some(parent))
}
