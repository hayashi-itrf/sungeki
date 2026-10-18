//! Join managed startup to durable speech ownership before any engine process starts.
use crate::{
    sbv2_originals::Binding,
    sbv2_startup::{self, Ready, Request},
    sessions::Sessions,
    speech_dispatch::{Dispatched, Reservation},
    speech_journal::Input,
    speech_state::invalid,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::sbv2::{Sbv2JobInput, canonical_endpoint};

/// Dropping preserves durable quarantine. The process owner performs best-effort
/// cleanup, but a dropped future never counts as confirmed execution completion.
pub(crate) struct Managed {
    pub ready: Ready,
    pub input: Sbv2JobInput,
    dispatched: Dispatched,
}
pub(crate) async fn start(
    shared: &Arc<Mutex<Sessions>>,
    job_id: &str,
    binding: Binding,
    cpu_only: bool,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<Managed, ApiError> {
    let reservation = Reservation::acquire(&mut *shared.lock().map_err(invalid)?, job_id)?;
    let input = match reservation.input() {
        Input::Sbv2(input) => input.clone(),
        Input::Local(_) | Input::Voicevox(_) | Input::Qwen(_) | Input::Irodori(_) => {
            return Err(invalid("SBV2専用の起動経路です"));
        }
    };
    let prepared = input.prepared().map_err(invalid)?;
    if canonical_endpoint(&binding.endpoint).map_err(invalid)? != input.endpoint() {
        return Err(invalid("SBV2の起動設定とジョブの接続先が一致しません"));
    }
    let progress = crate::speech_state::progress::Tracker::default();
    let request = Request {
        progress: progress.clone(),
        binding,
        catalog: input.catalog().map_err(invalid)?,
        settings: prepared.audio().settings().clone(),
        identity: input.identity().clone(),
        cpu_only,
    };
    // Loading models already owns the GPU. Persist PossiblyRunning BEFORE spawn,
    // not only before /voice, so interruption during startup remains quarantined.
    let dispatched = reservation.begin(&mut *shared.lock().map_err(invalid)?)?;
    if let Err(error) = progress.persist(shared, job_id) {
        dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
        return Err(error);
    }
    let startup = sbv2_startup::start(request, cancel.clone(), timeout);
    tokio::pin!(startup);
    let mut observation_error = None;
    let ready = loop {
        tokio::select! {
            result = &mut startup => break result,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if let Err(error) = progress.persist(shared, job_id) {
                    cancel.store(true, Ordering::Release);
                    observation_error.get_or_insert(error);
                }
            }
        }
    };
    match ready {
        Ok(ready) => {
            let managed = Managed {
                ready,
                input,
                dispatched,
            };
            let running = {
                let sessions = shared.lock().map_err(invalid)?;
                let stored = crate::speech_failure::load(&sessions, job_id)?;
                crate::speech_journal::decode_input(&stored)?.1.phase
                    == sungeki_project::JobPhase::Running
            };
            if !running || cancel.load(Ordering::Acquire) {
                managed.finish(shared).await?;
                let error = observation_error.unwrap_or_else(|| {
                    ApiError::new(ErrorCode::Cancelled, "SBV2起動中にジョブが取り消されました")
                });
                crate::speech_failure::record(shared, job_id, error.clone(), &cancel)?;
                return Err(error);
            }
            Ok(managed)
        }
        Err(failure) => {
            if failure.cleanup_confirmed {
                dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
            } else {
                drop(dispatched);
            }
            let mut error = observation_error.unwrap_or(failure.error);
            if let Some(cleanup) = failure.cleanup_error {
                error
                    .details
                    .insert("cleanup_error".into(), cleanup.to_string());
            }
            error.details.insert(
                "cleanup_confirmed".into(),
                failure.cleanup_confirmed.to_string(),
            );
            crate::speech_failure::record(shared, job_id, error.clone(), &cancel)?;
            // Failure retains the child until here; Drop is best effort, never
            // releases an unconfirmed journal/resource quarantine.
            Err(error)
        }
    }
}
impl Managed {
    /// Stop the owned process on a blocking worker, then persist Finished before
    /// releasing its lease. This does not mark WAV verified or the job successful.
    pub async fn finish(self, shared: &Arc<Mutex<Sessions>>) -> Result<(), ApiError> {
        let mut process = self.ready.process;
        let stopped = tokio::task::spawn_blocking(move || {
            let result = process.stop(Duration::from_secs(2));
            (process, result)
        })
        .await
        .map_err(|_| {
            ApiError::new(
                ErrorCode::EngineUnavailable,
                "SBV2停止の結果を確認できません",
            )
        })?;
        stopped.1.map_err(ApiError::io)?;
        self.dispatched
            .confirm_finished(&mut *shared.lock().map_err(invalid)?)
    }
}
