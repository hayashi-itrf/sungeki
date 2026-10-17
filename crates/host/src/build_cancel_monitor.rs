use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant},
};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::ApiError;
use sungeki_storage::JobKind;

struct Active(build::Runtime, String);
impl Drop for Active {
    fn drop(&mut self) {
        self.0
            .monitors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.1);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_one();
    }
}
pub(crate) fn launch(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    timeout: Duration,
) -> Result<bool, ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    let row = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作記録がありません"))?;
    if build::decode(&row)?.1.phase != JobPhase::Cancelling {
        return Ok(false);
    }
    let runtime = sessions.build_runtime.clone();
    if !runtime
        .monitors
        .lock()
        .map_err(invalid)?
        .insert(id.to_owned())
    {
        return Ok(false);
    }
    runtime.active.fetch_add(1, Ordering::SeqCst);
    let active = Active(runtime, id.to_owned());
    let shared = shared.clone();
    let id = id.to_owned();
    tokio::spawn(async move {
        let _active = active;
        let deadline = Instant::now() + timeout;
        loop {
            match observe(&shared, &id, Instant::now() >= deadline) {
                Ok(true) => break,
                Ok(false) => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(error) => {
                    eprintln!("build cancellation monitor {id}: {error}");
                    break;
                }
            }
        }
    });
    Ok(true)
}
fn observe(shared: &Arc<Mutex<Sessions>>, id: &str, expired: bool) -> Result<bool, ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let row = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作記録がありません"))?;
    let (input, mut state) = build::decode(&row)?;
    if state.phase != JobPhase::Cancelling || build::settle_cancel(&mut sessions, &row)? {
        return Ok(true);
    }
    if expired {
        state.progress = Some(Progress {
            stage: "取消要求済み・停止確認未完了（子処理と使用資源の確認が必要です）".into(),
            fraction: None,
        });
        state.validate(&input)?;
        sessions
            .store
            .update_job(
                id,
                JobKind::BuildVideo,
                row.version,
                &serde_json::to_vec(&state).map_err(invalid)?,
            )
            .map_err(storage_error)?;
        return Ok(true);
    }
    Ok(false)
}
