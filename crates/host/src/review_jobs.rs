#[path = "review_start.rs"]
mod start;
use crate::{
    review_record::{State, decode, invalid},
    sessions::{Sessions, storage_error},
};
pub(crate) use start::{start, start_bound};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use sungeki_protocol::*;
use sungeki_storage::{JobKind, StoredJob};

#[derive(Clone)]
pub(crate) struct Runtime {
    pub freshness: Arc<crate::review_freshness::Cache>,
    pub cancellations: Arc<Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>>,
    pub idle_wake: Arc<tokio::sync::Notify>,
    pub active: Arc<AtomicUsize>,
    pub slots: Arc<tokio::sync::Semaphore>,
    signal: Arc<(Mutex<u64>, Condvar)>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            freshness: Default::default(),
            cancellations: Default::default(),
            idle_wake: Arc::new(tokio::sync::Notify::new()),
            active: Arc::new(AtomicUsize::new(0)),
            slots: Arc::new(tokio::sync::Semaphore::new(1)),
            signal: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }
}
impl Runtime {
    pub fn notify(&self) {
        self.idle_wake.notify_one();
        if let Ok(mut generation) = self.signal.0.lock() {
            *generation = generation.wrapping_add(1);
            self.signal.1.notify_all();
        }
    }
}
pub(crate) fn recover(sessions: &mut Sessions) -> Result<(), ApiError> {
    let mut after = None;
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::Review), None, after.as_deref(), 100)
            .map_err(storage_error)?;
        if ids.is_empty() {
            break;
        }
        for id in &ids {
            let job = sessions
                .store
                .load_job(id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("ジョブ欠落"))?;
            // Finished records are immutable history, not recovery work. Their
            // input is validated on retrieval; an unreadable historical Project
            // must not prevent opening unrelated documents at host startup.
            let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
            state.lifecycle()?;
            if state.phase.is_terminal() {
                continue;
            }
            let (_, mut state) = decode(&job)?;
            let mut changed = false;
            if !state.phase.is_terminal() {
                state.update(
                    sungeki_jobs::Lifecycle::recover(state.phase, state.progress.clone())
                        .map_err(invalid)?,
                );
                changed = true;
            }
            if let Ok(root) = &sessions.review_directory {
                if let Err(error) = crate::review_workdir::recover(root, id) {
                    state.error = Some(error);
                    changed = true;
                }
            }
            if changed {
                save(sessions, &job, &state)?;
            }
        }
        after = ids.last().cloned();
    }
    Ok(())
}
pub(crate) fn save(
    sessions: &mut Sessions,
    record: &StoredJob,
    state: &State,
) -> Result<(), ApiError> {
    decode(&StoredJob {
        state: serde_json::to_vec(state).map_err(invalid)?,
        ..record.clone()
    })?;
    sessions
        .store
        .update_job(
            &record.id,
            JobKind::Review,
            record.version,
            &serde_json::to_vec(state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    Ok(())
}
pub(crate) fn record(sessions: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "ジョブが見つかりません"))
}
pub(crate) fn result(sessions: &Sessions, id: &str) -> Result<ReviewJob, ApiError> {
    let job = record(sessions, id)?;
    let (input, state) = decode(&job)?;
    let (_, current) = sessions.load(&job.session_id)?;
    let asset_freshness = sessions.review_runtime.freshness.observe(
        &sessions.review_runtime,
        id,
        &input.project.assets,
        state
            .report
            .as_ref()
            .and_then(|report| report.asset_fingerprint.as_deref()),
    );
    let stale = current.history().revision() != input.source_revision
        || sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
            != input.content_hash
        || asset_freshness.last_known_changed
        || has_outdated_images(state.report.as_ref());
    Ok(ReviewJob {
        job_id: job.id,
        session_id: job.session_id,
        source_revision: input.source_revision,
        content_hash: input.content_hash,
        phase: state.phase,
        progress: state.progress,
        report: state.report,
        error: state.error,
        stale,
        asset_freshness,
    })
}
// Structure-only and unsuccessful image checks have no old visual result to reuse.
pub(crate) fn has_outdated_images(report: Option<&ReviewReport>) -> bool {
    report
        .and_then(|report| report.representative_frames.as_ref())
        .is_some_and(|frames| {
            frames
                .frames
                .iter()
                .filter_map(|frame| frame.output.as_ref())
                .any(|output| output.layout_version != sungeki_render::CURRENT_LAYOUT_VERSION)
        })
}

pub(crate) fn get(
    shared: &Arc<Mutex<Sessions>>,
    id: String,
    wait_ms: u32,
) -> Result<SessionResponse, ApiError> {
    if wait_ms > 30000 {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "待機は0〜30000msです",
        ));
    }
    let runtime = shared.lock().map_err(invalid)?.review_runtime.clone();
    let deadline = Instant::now() + Duration::from_millis(wait_ms as u64);
    loop {
        let generation = runtime.signal.0.lock().map_err(invalid)?;
        let job = result(&*shared.lock().map_err(invalid)?, &id)?;
        if job.phase.is_terminal() || Instant::now() >= deadline {
            return Ok(SessionResponse::ReviewJob { job });
        }
        let _guard = runtime
            .signal
            .1
            .wait_timeout(
                generation,
                deadline.saturating_duration_since(Instant::now()),
            )
            .map_err(invalid)?;
    }
}
pub(crate) fn cancel(
    shared: &Arc<Mutex<Sessions>>,
    id: String,
) -> Result<SessionResponse, ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let job = record(&sessions, &id)?;
    let (_, mut state) = decode(&job)?;
    let mut lifecycle = state.lifecycle()?;
    if lifecycle.request_cancel() {
        state.update(lifecycle);
        save(&mut sessions, &job, &state)?;
    }
    let runtime = sessions.review_runtime.clone();
    drop(sessions);
    if let Some(flag) = runtime
        .cancellations
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&id)
    {
        flag.store(true, Ordering::SeqCst);
    }
    runtime.notify();
    get(shared, id, 0)
}
