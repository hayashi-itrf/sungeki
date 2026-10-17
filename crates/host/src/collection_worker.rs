//! Background copy and publication. Editor locks cover journal writes only.
use super::{Error, execute_journaled, record};
use crate::{media_job_runtime::Runtime, sessions::Sessions};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use sungeki_project::JobPhase;
use sungeki_storage::{PublishError, StoredJob};
pub(super) fn invalid(e: impl std::fmt::Debug) -> Error {
    Error::Invalid(format!("収集実行: {e:?}"))
}
pub(super) fn load(sessions: &Sessions, id: &str) -> Result<StoredJob, Error> {
    sessions
        .store
        .load_job(id)
        .map_err(invalid)?
        .filter(|job| job.kind == sungeki_storage::JobKind::Collection)
        .ok_or_else(|| {
            Error::Api(sungeki_protocol::ApiError::new(
                sungeki_protocol::ErrorCode::NotFound,
                "収集ジョブがありません",
            ))
        })
}
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
/// Caller has registered the flag and active count before releasing session lock.
pub(super) fn launch(
    shared: Arc<Mutex<Sessions>>,
    runtime: Runtime,
    id: String,
    flag: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let _active = Active(runtime.clone(), id.clone());
        let Ok(_slot) = runtime.slots.clone().acquire_owned().await else {
            return;
        };
        let work_shared = shared.clone();
        let work_id = id.clone();
        let result = tokio::task::spawn_blocking(move || run(&work_shared, &work_id, &flag)).await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("collection {id}: {e:?}"),
            Err(e) => eprintln!("collection {id}: {e}"),
        }
        // Incomplete persistence remains recoverable; never retry publication here.
    });
}
fn run(shared: &Arc<Mutex<Sessions>>, id: &str, flag: &AtomicBool) -> Result<(), Error> {
    let input = {
        let mut sessions = shared.lock().map_err(invalid)?;
        let job = load(&sessions, id)?;
        let (input, mut state) = record::decode(&job)?;
        if state.phase.is_terminal() {
            return Ok(());
        }
        if state.phase == JobPhase::Cancelling || flag.load(Ordering::SeqCst) {
            state.failed_before_publication(
                true,
                super::errors::api(PublishError::Cancelled.into()),
            )?;
            record::save_and_notify(&mut sessions, &job, &state)?;
            return Ok(());
        }
        state.start()?;
        record::save_and_notify(&mut sessions, &job, &state)?;
        input
    };
    let deadline = Instant::now() + Duration::from_secs(7200);
    let cancelled = || flag.load(Ordering::SeqCst) || Instant::now() >= deadline;
    if input.dry_run {
        return super::dry_run::run(shared, id, &input, cancelled);
    }
    let result = execute_journaled(
        &input.project,
        &input.checkpoints,
        &input.destination,
        sungeki_assets::CollectionLimits::default(),
        &input.collected_project_id,
        cancelled,
        |stage| {
            let mut sessions = shared.lock().map_err(invalid)?;
            let job = load(&sessions, id)?;
            let (_, mut state) = record::decode(&job)?;
            if state.phase == JobPhase::Cancelling || cancelled() {
                return Err(PublishError::Cancelled.into());
            }
            state.stage_created(stage)?;
            record::save_and_notify(&mut sessions, &job, &state)
        },
        |receipt| {
            let mut sessions = shared.lock().map_err(invalid)?;
            let job = load(&sessions, id)?;
            let (_, mut state) = record::decode(&job)?;
            if state.phase == JobPhase::Cancelling || cancelled() {
                return Err(PublishError::Cancelled.into());
            }
            state.intent(receipt)?;
            record::save_and_notify(&mut sessions, &job, &state)
        },
    );
    // DurabilityUncertain means rename already occurred. Verify the published
    // receipt outside the lock; never report a late cancel as a reverted output.
    let uncertain = matches!(
        result,
        Err(Error::Publish(PublishError::DurabilityUncertain { .. }))
    );
    let recovered = if uncertain {
        let observed = {
            let sessions = shared.lock().map_err(invalid)?;
            load(&sessions, id)?
        };
        Some((observed.clone(), record::recover(&observed, cancelled)?))
    } else {
        None
    };
    let mut sessions = shared.lock().map_err(invalid)?;
    if let Some((observed, state)) = recovered {
        return record::save_and_notify(&mut sessions, &observed, &state);
    }
    let job = load(&sessions, id)?;
    let (_, mut state) = record::decode(&job)?;
    match result {
        Ok(receipt) => state.published(&receipt)?,
        Err(error) => {
            let is_cancel = matches!(
                error,
                Error::Publish(PublishError::Cancelled)
                    | Error::Plan(sungeki_assets::CollectionError::Cancelled)
            );
            state.failed_before_publication(is_cancel, super::errors::api(error))?;
        }
    }
    record::save_and_notify(&mut sessions, &job, &state)
}
