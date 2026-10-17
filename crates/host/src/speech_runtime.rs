//! Bounded in-process ownership. Durable records remain the source of truth.
use crate::{
    sessions::{Sessions, storage_error},
    speech_journal::{Input, create_input, decode_input as decode, save},
    speech_state::invalid,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::SpeechJobInput;
use sungeki_storage::StoredJob;

#[derive(Default)]
struct Control {
    cancelled: Arc<AtomicBool>,
    wake: tokio::sync::Notify,
}
#[derive(Clone)]
pub(crate) struct Runtime {
    pub(crate) changed: Arc<(Mutex<u64>, std::sync::Condvar)>,
    pub(crate) preparations: crate::speech_preparation::Preparations,
    controls: Arc<Mutex<HashMap<String, Arc<Control>>>>,
    pub active: Arc<AtomicUsize>,
    pub idle_wake: Arc<tokio::sync::Notify>,
    slots: Arc<tokio::sync::Semaphore>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            changed: Default::default(),
            preparations: Default::default(),
            controls: Default::default(),
            active: Default::default(),
            idle_wake: Default::default(),
            slots: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
struct Active {
    runtime: Runtime,
    id: String,
}
impl Drop for Active {
    fn drop(&mut self) {
        self.runtime
            .controls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
        self.runtime.active.fetch_sub(1, Ordering::SeqCst);
        self.runtime.idle_wake.notify_one();
    }
}
/// Replays return the durable record without dispatch, including after completion
/// or unrelated Project edits. New requests are limited to eight active/waiting jobs.
pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    id: String,
    input: &SpeechJobInput,
) -> Result<StoredJob, ApiError> {
    start_input(
        shared,
        session_id,
        id,
        &Input::Voicevox(input.clone()),
        crate::speech_worker::Worker::Voicevox,
    )
}
/// An explicit, already selected launch binding is required. No implicit local
/// server discovery or fallback to a different speech engine.
pub(crate) fn start_sbv2(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    id: String,
    input: &sungeki_speech::sbv2::Sbv2JobInput,
    binding: crate::sbv2_originals::Binding,
    cpu_only: bool,
) -> Result<StoredJob, ApiError> {
    start_input(
        shared,
        session_id,
        id,
        &Input::Sbv2(input.clone()),
        crate::speech_worker::Worker::Sbv2 { binding, cpu_only },
    )
}
pub(crate) fn start_qwen(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    id: String,
    input: &sungeki_speech::qwen::QwenJobInput,
    binding: crate::qwen_originals::binding::Binding,
    port: u16,
) -> Result<StoredJob, ApiError> {
    start_input(
        shared,
        session_id,
        id,
        &Input::Qwen(input.clone()),
        crate::speech_worker::Worker::Qwen { binding, port },
    )
}
pub(crate) fn start_irodori(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    id: String,
    input: &sungeki_speech::irodori::IrodoriJobInput,
    binding: crate::irodori_originals::binding::Binding,
    port: u16,
) -> Result<StoredJob, ApiError> {
    start_input(
        shared,
        session_id,
        id,
        &Input::Irodori(input.clone()),
        crate::speech_worker::Worker::Irodori { binding, port },
    )
}
fn start_input(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    id: String,
    input: &Input,
    worker: crate::speech_worker::Worker,
) -> Result<StoredJob, ApiError> {
    start_prepared(shared, session_id, id, input, worker, None)
}
pub(crate) fn start_prepared(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    id: String,
    input: &Input,
    worker: crate::speech_worker::Worker,
    preparation: Option<crate::speech_preparation::Preparation>,
) -> Result<StoredJob, ApiError> {
    let handle = tokio::runtime::Handle::try_current().map_err(invalid)?;
    let mut sessions = shared.lock().map_err(invalid)?;
    if sessions
        .store
        .load_job(&id)
        .map_err(storage_error)?
        .is_some()
    {
        return create_input(&mut sessions, id, session_id, input);
    }
    worker.validate(input)?;
    let runtime = sessions.speech_runtime.clone();
    if runtime.active.load(Ordering::SeqCst) >= 8 {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "音声ジョブが混み合っています",
        ));
    }
    let _preparation = match preparation {
        Some(held) => runtime.preparations.adopt(held, &id, &session_id)?,
        None => runtime
            .preparations
            .claim(&id, &session_id, input.input_hash())?,
    };
    // Media directory I/O must not block unrelated editor/IPC operations.
    let database = sessions.review_database_path.clone();
    drop(sessions);
    let directory = crate::speech_artifacts::root(&database)?;
    let mut sessions = shared.lock().map_err(invalid)?;
    // Another start or edit may have committed while preparing the directory.
    if sessions
        .store
        .load_job(&id)
        .map_err(storage_error)?
        .is_some()
    {
        return create_input(&mut sessions, id, session_id, input);
    }
    if runtime.active.load(Ordering::SeqCst) >= 8 {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "音声ジョブが混み合っています",
        ));
    }
    sessions.speech_directory = Ok(directory);
    let mut controls = runtime.controls.lock().map_err(invalid)?;
    let job = create_input(&mut sessions, id.clone(), session_id, input)?;
    let control = Arc::new(Control::default());
    controls.insert(id.clone(), control.clone());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(controls);
    drop(sessions);
    let active = Active {
        runtime,
        id: id.clone(),
    };
    let shared = shared.clone();
    handle.spawn(async move {
        let _active = active;
        let runtime = &_active.runtime;
        // The supervisor observes panic/abort in the worker. Dropped dispatch
        // leases quarantine external work; failure recording never releases them.
        let worker_shared = shared.clone();
        let worker_id = id.clone();
        let worker_control = control.clone();
        let slots = runtime.slots.clone();
        let result = tokio::spawn(async move {
            let _slot = tokio::select! {
                slot = slots.acquire_owned() => slot.map_err(invalid)?,
                _ = worker_control.wake.notified() => return Err(cancel_error()),
            };
            if worker_control.cancelled.load(Ordering::Acquire) {
                return Err(cancel_error());
            }
            worker
                .run(&worker_shared, &worker_id, worker_control.cancelled.clone())
                .await
        })
        .await
        .map_err(invalid)
        .and_then(|value| value);
        if let Err(error) = result {
            if let Err(persist_error) =
                crate::speech_failure::record(&shared, &id, error, &control.cancelled)
            {
                eprintln!("speech job {id}: {persist_error}");
            }
        }
    });
    Ok(job)
}
/// Persist cancellation before notifying the worker. Unsent queued work can be
/// acknowledged immediately; dispatched work must drain/settle the engine first.
pub(crate) fn cancel(shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<StoredJob, ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let mut record = crate::speech_failure::load(&sessions, id)?;
    let (_, mut state) = decode(&record)?;
    if !state.phase.is_terminal() {
        let unsent = state.phase == JobPhase::Queued;
        if state.phase != JobPhase::Cancelling {
            state.phase = JobPhase::Cancelling;
            save(&mut sessions, &record, &state)?;
            record = crate::speech_failure::load(&sessions, id)?;
        }
        if unsent {
            state.phase = JobPhase::Cancelled;
            state.error = Some(cancel_error());
            save(&mut sessions, &record, &state)?;
        }
        if let Some(control) = sessions
            .speech_runtime
            .controls
            .lock()
            .map_err(invalid)?
            .get(id)
        {
            control.cancelled.store(true, Ordering::Release);
            control.wake.notify_one();
        }
        sessions.speech_runtime.idle_wake.notify_one();
    }
    crate::speech_failure::load(&sessions, id)
}
fn cancel_error() -> ApiError {
    ApiError::new(ErrorCode::Cancelled, "音声ジョブをキャンセルしました")
}

#[cfg(test)]
#[path = "speech_runtime_tests.rs"]
mod tests;

impl Runtime {
    pub(crate) fn owns(&self, id: &str) -> Result<bool, ApiError> {
        Ok(self.preparations.contains(id)?
            || self.controls.lock().map_err(invalid)?.contains_key(id))
    }
}

impl Runtime {
    pub(crate) fn notify_changed(&self) {
        let mut generation = self.changed.0.lock().unwrap_or_else(|e| e.into_inner());
        *generation = generation.wrapping_add(1);
        self.changed.1.notify_all();
    }
}

pub(crate) fn cancel_all(shared: &Arc<Mutex<Sessions>>) -> Result<(), ApiError> {
    let ids: Vec<String> = shared
        .lock()
        .map_err(invalid)?
        .speech_runtime
        .controls
        .lock()
        .map_err(invalid)?
        .keys()
        .cloned()
        .collect();
    for id in ids {
        cancel(shared, &id)?;
    }
    Ok(())
}
