//! Store and verify a received original, then atomically apply matching results.
use crate::{
    sessions::Sessions,
    speech_failure,
    speech_journal::decode_input as decode,
    speech_state::{Application, Execution, invalid},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::SpeechResponse;

pub(crate) async fn complete(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    response: impl Into<SpeechResponse>,
    cancelled: &AtomicBool,
) -> Result<(), ApiError> {
    let response = response.into();
    let (root, settings) = {
        let sessions = shared.lock().map_err(invalid)?;
        let record = speech_failure::load(&sessions, id)?;
        let (input, state) = decode(&record)?;
        if !input.accepts_response(&response)? {
            return Err(invalid(
                "音声応答のエンジンまたは固定入力がジョブと一致しません",
            ));
        }
        if state.phase == JobPhase::Cancelling || cancelled.load(Ordering::Acquire) {
            drop(sessions);
            return cancel(shared, id, cancelled);
        }
        if state.phase != JobPhase::Running || state.execution != Execution::Finished {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "音声成果物を登録できる状態ではありません",
            ));
        }
        (sessions.speech_directory.clone(), input.lip_sync_settings())
    };
    let id_owned = id.to_owned();
    let stored = match root {
        Ok(root) => {
            let flag = Arc::new(AtomicBool::new(false));
            let guard = AnalysisCancel(flag.clone());
            let mut task = tokio::task::spawn_blocking(move || {
                crate::speech_artifacts::store(&root, &id_owned, response, settings, &flag)
            });
            loop {
                tokio::select! {
                    result = &mut task => break result.map_err(invalid).and_then(|result| result),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                        if cancelled.load(Ordering::Acquire) { guard.0.store(true, Ordering::Release); }
                    }
                }
            }
        }
        Err(error) => Err(error),
    };
    let artifact = match stored {
        Ok(artifact) => artifact,
        Err(error) => {
            speech_failure::record(shared, id, error.clone(), cancelled)?;
            return Err(error);
        }
    };
    let mut sessions = shared.lock().map_err(invalid)?;
    let record = speech_failure::load(&sessions, id)?;
    let (_, mut state) = decode(&record)?;
    if state.phase == JobPhase::Cancelling || cancelled.load(Ordering::Acquire) {
        drop(sessions);
        return cancel(shared, id, cancelled);
    }
    if state.phase != JobPhase::Running || state.execution != Execution::Finished {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "保存中に音声ジョブが変更されました",
        ));
    }
    state.phase = JobPhase::Succeeded;
    state.artifact = Some(artifact);
    state.application = Application::PendingConflict;
    crate::speech_apply::finish(&mut sessions, &record, state)
}
fn cancel(shared: &Arc<Mutex<Sessions>>, id: &str, cancelled: &AtomicBool) -> Result<(), ApiError> {
    let error = ApiError::new(ErrorCode::Cancelled, "音声成果物の登録をキャンセルしました");
    speech_failure::record(shared, id, error.clone(), cancelled)?;
    Err(error)
}
pub(crate) async fn run(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    cancelled: &AtomicBool,
    timeout: std::time::Duration,
) -> Result<(), ApiError> {
    let response = crate::speech_exchange::run(shared, id, cancelled, timeout).await?;
    complete(shared, id, response, cancelled).await
}

// Dropping the async owner also signals its blocking analysis to stop.
struct AnalysisCancel(Arc<AtomicBool>);
impl Drop for AnalysisCancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
