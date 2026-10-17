use super::proxy_record::{decode, invalid, record, save};
use crate::{media_job_runtime::Runtime, sessions::Sessions};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::{ApiError, ErrorCode};
struct Active(Runtime, String);
impl Drop for Active {
    fn drop(&mut self) {
        self.0
            .flags
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.1);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_waiters();
    }
}
/// Caller must persist the job and register active/flag before launch.
pub(crate) fn launch(
    shared: Arc<Mutex<Sessions>>,
    runtime: Runtime,
    id: String,
    flag: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let _active = Active(runtime.clone(), id.clone());
        if let Err(error) = run(shared, runtime, &id, flag).await {
            eprintln!("proxy job {id}: {error}");
        }
    });
}
async fn run(
    shared: Arc<Mutex<Sessions>>,
    runtime: Runtime,
    id: &str,
    flag: Arc<AtomicBool>,
) -> Result<(), ApiError> {
    loop {
        let input = {
            let mut sessions = shared.lock().map_err(invalid)?;
            let job = record(&sessions, id)?;
            let (input, mut state) = decode(&job)?;
            let mut lifecycle = state.lifecycle()?;
            if state.phase.is_terminal() {
                return Ok(());
            }
            if flag.load(Ordering::Acquire)
                || runtime.stopping.load(Ordering::Acquire)
                || state.phase == JobPhase::Cancelling
            {
                lifecycle.request_cancel();
                lifecycle.acknowledge_cancel().map_err(invalid)?;
                state.update(lifecycle);
                save(&mut sessions, &job, &state)?;
                return Ok(());
            }
            if state.phase == JobPhase::Queued {
                lifecycle.start().map_err(invalid)?;
            }
            lifecycle
                .update_progress(Progress {
                    stage: "軽量動画の生成待ち".into(),
                    fraction: Some(state.completed.len() as f64 / input.asset_ids.len() as f64),
                })
                .map_err(invalid)?;
            state.update(lifecycle);
            save(&mut sessions, &job, &state)?;
            let next = input
                .asset_ids
                .iter()
                .find(|asset| {
                    !state
                        .completed
                        .iter()
                        .any(|output| &output.asset_id == *asset)
                })
                .cloned()
                .ok_or_else(|| invalid("次の動画がありません"))?;
            (input, next)
        };
        let Some(slot) = runtime
            .slots
            .clone()
            .acquire_background(&flag, &runtime.stopping)
            .await
        else {
            continue;
        };
        {
            let mut sessions = shared.lock().map_err(invalid)?;
            let job = record(&sessions, id)?;
            let (_, mut state) = decode(&job)?;
            if state.phase != JobPhase::Running
                || flag.load(Ordering::Acquire)
                || runtime.stopping.load(Ordering::Acquire)
            {
                drop(slot);
                continue;
            }
            let mut lifecycle = state.lifecycle()?;
            lifecycle
                .update_progress(Progress {
                    stage: "軽量動画を生成・検証中".into(),
                    fraction: Some(state.completed.len() as f64 / input.0.asset_ids.len() as f64),
                })
                .map_err(invalid)?;
            state.update(lifecycle);
            save(&mut sessions, &job, &state)?;
        }
        let interrupt = slot.interrupted.clone();
        let stopped = runtime.stopping.clone();
        let cancelled = flag.clone();
        let result = tokio::task::spawn_blocking(move || {
            super::cancellation::run(
                &stopped,
                || cancelled.load(Ordering::Acquire) || interrupt.load(Ordering::Acquire),
                |local| super::proxy_execute::execute(&input.0, &input.1, local),
            )
        })
        .await
        .map_err(invalid)
        .and_then(|v| v);
        let preempted = slot.interrupted.load(Ordering::Acquire);
        drop(slot); // worker and all temporary snapshots have settled before yielding.
        let mut sessions = shared.lock().map_err(invalid)?;
        let job = record(&sessions, id)?;
        let (input, mut state) = decode(&job)?;
        let mut lifecycle = state.lifecycle()?;
        if state.phase == JobPhase::Cancelling
            || flag.load(Ordering::Acquire)
            || runtime.stopping.load(Ordering::Acquire)
        {
            lifecycle.request_cancel();
            lifecycle.acknowledge_cancel().map_err(invalid)?;
        } else if preempted
            && result
                .as_ref()
                .is_err_and(|e| e.code == ErrorCode::Cancelled)
        {
            lifecycle
                .update_progress(Progress {
                    stage: "書き出しなどの処理を待っています".into(),
                    fraction: Some(state.completed.len() as f64 / input.asset_ids.len() as f64),
                })
                .map_err(invalid)?;
        } else {
            match result {
                Ok(output) => {
                    state.completed.push(output);
                    let done = state.completed.len() == input.asset_ids.len();
                    lifecycle
                        .update_progress(Progress {
                            stage: if done {
                                "軽量動画の検証完了"
                            } else {
                                "軽量動画を生成中"
                            }
                            .into(),
                            fraction: Some(
                                state.completed.len() as f64 / input.asset_ids.len() as f64,
                            ),
                        })
                        .map_err(invalid)?;
                    if done {
                        lifecycle.finish(true).map_err(invalid)?;
                    }
                }
                Err(error) => {
                    lifecycle.finish(false).map_err(invalid)?;
                    state.error = Some(error);
                }
            }
        }
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
        if state.phase.is_terminal() {
            return Ok(());
        }
    }
}
