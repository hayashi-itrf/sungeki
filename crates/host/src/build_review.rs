use crate::{build_record::decode, build_state::invalid, sessions::Sessions};
use std::sync::{Arc, Mutex};
use sungeki_protocol::{ApiError, SessionResponse};
use sungeki_storage::StoredJob;

pub(crate) fn validate_parent(
    sessions: &Sessions,
    parent_id: &str,
    child_id: &str,
    session_id: &str,
    revision: u32,
) -> Result<String, ApiError> {
    super::render::validate_step(sessions, parent_id, child_id, session_id, revision, 7)
}
/// Blocking file verification before dispatch; never called under the session lock.
pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    parent: &StoredJob,
) -> Result<SessionResponse, ApiError> {
    crate::build_record::accept_render(shared, parent)?;
    let (_, state) = decode(parent)?;
    crate::review_jobs::start_bound(
        shared,
        parent.session_id.clone(),
        state
            .child_id
            .ok_or_else(|| invalid("確認工程が未予約です"))?,
        state.expected_revision,
        Some(parent),
        Default::default(),
    )
}
