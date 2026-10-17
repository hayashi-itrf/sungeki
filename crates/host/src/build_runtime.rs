//! In-process parent ownership. Only queued parents may acquire a new owner;
//! interrupted/running journals are never evidence to automatically resume.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::{
    collections::HashSet,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::JobKind;

#[derive(Clone, Default)]
pub(crate) struct Runtime {
    pub changed: Arc<(Mutex<u64>, Condvar)>,
    owners: Arc<Mutex<HashSet<String>>>,
    pub(crate) stopping: Arc<AtomicBool>,
    pub(super) monitors: Arc<Mutex<HashSet<String>>>,
    pub active: Arc<AtomicUsize>,
    pub wake: Arc<tokio::sync::Notify>,
}
impl Runtime {
    pub fn notify_terminal(&self) {
        let mut generation = self.changed.0.lock().unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.changed.1.notify_all();
    }
}
struct Owner(Runtime, String);
impl Drop for Owner {
    fn drop(&mut self) {
        self.0
            .owners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.1);
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.wake.notify_one();
    }
}
/// Returns false for an already-owned parent. Connection is fixed for this owner.
/// API/shutdown wiring waits for the cancellation/resource-settlement layer.
pub(crate) fn launch(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    speech: Option<build::SpeechConnection>,
) -> Result<bool, ApiError> {
    let (owner, speech) = {
        let sessions = shared.lock().map_err(invalid)?;
        let row = sessions
            .store
            .load_job(id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("制作記録がありません"))?;
        let (input, state) = build::decode(&row)?;
        if let Some(source) = &input.continuation {
            source.verify_current(&sessions, id)?;
        }
        let speech = speech.map(|e| e.canonical(&input.recipe)).transpose()?;
        let speech = match (input.engine, speech) {
            (Some(fixed), Some(given)) if fixed != given => {
                return Err(invalid("保存済み合成先を変更できません"));
            }
            (Some(fixed), _) => Some(fixed),
            (None, supplied) => supplied,
        };
        let runtime = sessions.build_runtime.clone();
        if runtime.stopping.load(Ordering::SeqCst) {
            return Err(invalid("host終了中のため制作を開始できません"));
        }
        let mut owners = runtime.owners.lock().map_err(invalid)?;
        if owners.contains(id) {
            return Ok(false);
        }
        if state.phase != JobPhase::Queued {
            return Err(invalid(
                "新規待機中の制作だけを開始できます。中断・実行中の制作は自動再開しません",
            ));
        }
        if runtime.active.load(Ordering::SeqCst) != 0 {
            return Err(ApiError::new(
                ErrorCode::CapabilityUnavailable,
                "別の一括制作が実行中です",
            ));
        }
        owners.insert(id.into());
        runtime.active.fetch_add(1, Ordering::SeqCst);
        drop(owners);
        (Owner(runtime, id.into()), speech)
    };
    let shared = shared.clone();
    let id = id.to_owned();
    tokio::spawn(async move {
        let _owner = owner;
        loop {
            match build::advance(&shared, &id, speech.clone()).await {
                Ok(build::Advance::Progress) => tokio::task::yield_now().await,
                Ok(build::Advance::Waiting) => tokio::time::sleep(Duration::from_millis(100)).await,
                Ok(build::Advance::Stopped) => break,
                Err(error) => {
                    if let Err(persist) = interrupt(&shared, &id, error) {
                        eprintln!("build job {id}: {persist}");
                    }
                    break;
                }
            }
        }
        if let Err(error) = notify_cancel(&shared, &id) {
            eprintln!("build cancellation {id}: {error}");
        }
    });
    Ok(true)
}
fn interrupt(shared: &Arc<Mutex<Sessions>>, id: &str, error: ApiError) -> Result<(), ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let row = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作記録がありません"))?;
    let (input, mut state) = build::decode(&row)?;
    if !matches!(state.phase, JobPhase::Queued | JobPhase::Running) {
        return Ok(());
    }
    // Preserve child ownership and receipts. An error is not proof that an
    // uncertain child/external engine is stopped or its resources are released.
    state.phase = JobPhase::Interrupted;
    state.error = Some(error);
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
    sessions.build_runtime.notify_terminal();
    Ok(())
}

fn notify_cancel(shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<(), ApiError> {
    let row = shared
        .lock()
        .map_err(invalid)?
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作記録がありません"))?;
    if build::decode(&row)?.1.phase == JobPhase::Cancelling {
        build::cancel_owned(shared, &row)?;
        build::monitor_cancel(shared, id, Duration::from_secs(30))?;
    }
    Ok(())
}

impl Runtime {
    pub(crate) fn owns(&self, id: &str) -> Result<bool, ApiError> {
        Ok(self.owners.lock().map_err(invalid)?.contains(id))
    }
}
