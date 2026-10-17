//! Cooperatively stop blocking original inspection, then await its completion.
use crate::{
    build_record,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::StoredJob;
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
fn current(shared: &Arc<Mutex<Sessions>>, expected: &StoredJob) -> Result<bool, ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    if sessions.build_runtime.stopping.load(Ordering::Acquire) {
        return Ok(false);
    }
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
    {
        return Ok(false);
    }
    let (_, state) = build_record::decode(expected)?;
    let (stored, _) = sessions.load(&expected.session_id)?;
    Ok(state.phase == sungeki_project::JobPhase::Running
        && stored.revision == state.expected_revision)
}
fn cancelled() -> ApiError {
    ApiError::new(ErrorCode::Cancelled, "制作音声の準備を中止しました")
}

pub(crate) async fn run<T: Send + 'static>(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
    work: impl FnOnce(Arc<AtomicBool>) -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    if !current(shared, expected)? {
        release_unstarted(shared, expected)?;
        return Err(cancelled());
    }
    let flag = Arc::new(AtomicBool::new(false));
    let guard = CancelOnDrop(flag.clone());
    let mut task = tokio::task::spawn_blocking(move || work(flag));
    let mut check_error = None;
    loop {
        tokio::select! {
            result = &mut task => {
                let result = result.map_err(invalid)?;
                release_unstarted(shared, expected)?;
                if let Some(error) = check_error { return Err(error); }
                if guard.0.load(Ordering::Acquire) { return Err(cancelled()); }
                return result;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                match current(shared, expected) {
                    Ok(true) => {},
                    Ok(false) => guard.0.store(true, Ordering::Release),
                    Err(error) => {
                        guard.0.store(true, Ordering::Release);
                        check_error = Some(error);
                    }
                }
            }
        }
    }
}

// Only the fresh dispatch path calls run. Reaching here proves its blocking
// function returned (or never began), not merely that its observer disappeared.
// A panic/lost worker bypasses this release and retains the uncertain reservation.
pub(crate) fn release_unstarted(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<(), ApiError> {
    release(shared, expected, false)
}
/// Only for a returned local reuse task: no external synthesis was dispatched.
pub(crate) fn release_local_unstarted(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<(), ApiError> {
    release(shared, expected, true)
}
fn release(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
    local: bool,
) -> Result<(), ApiError> {
    let sessions = &mut *shared.lock().map_err(invalid)?;
    let row = sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作記録がありません"))?;
    let (input, mut state) = build_record::decode(&row)?;
    let (_, reserved) = build_record::decode(expected)?;
    if (state.phase != sungeki_project::JobPhase::Cancelling
        && !(local && state.phase == sungeki_project::JobPhase::Running))
        || row.input != expected.input
        || row.session_id != expected.session_id
        || state.child_id != reserved.child_id
    {
        return Ok(());
    }
    let Some(id) = &state.child_id else {
        return Ok(());
    };
    if sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .is_some()
        || sessions.speech_runtime.owns(id)?
        || sessions.speech_resources.has_owner(id).map_err(invalid)?
    {
        return Ok(());
    }
    state.child_id = None;
    state.validate(&input)?;
    sessions
        .store
        .update_job(
            &row.id,
            sungeki_storage::JobKind::BuildVideo,
            row.version,
            &serde_json::to_vec(&state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    Ok(())
}
