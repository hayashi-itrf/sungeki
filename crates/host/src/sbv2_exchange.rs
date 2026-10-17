//! Synthesis under managed process + durable dispatch ownership, followed by the
//! same original-WAV validation/storage/application path as other speech engines.
use crate::{
    sbv2_job::{self, Managed},
    sbv2_originals::Binding,
    sessions::Sessions,
    speech_state::invalid,
    speech_state::progress::{Stage, Tracker},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::{
    SpeechResponse,
    sbv2::{Error, Sbv2Client},
};

pub(crate) async fn run(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    binding: Binding,
    cpu_only: bool,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(), ApiError> {
    let result = exchange(shared, id, binding, cpu_only, cancel.clone(), timeout).await;
    match result {
        Ok(response) => crate::speech_output::complete(shared, id, response, &cancel).await,
        Err(error) => {
            crate::speech_failure::record(shared, id, error.clone(), &cancel)?;
            Err(error)
        }
    }
}
async fn exchange(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    binding: Binding,
    cpu_only: bool,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<SpeechResponse, ApiError> {
    // Reject invalid HTTP bounds before model loading starts.
    if timeout.is_zero() || timeout > Duration::from_secs(300) {
        return Err(invalid("SBV2の要求待機時間が不正です"));
    }
    let managed = sbv2_job::start(
        shared,
        id,
        binding.clone(),
        cpu_only,
        cancel.clone(),
        timeout,
    )
    .await?;
    let response = synthesize(shared, id, &managed, binding, cancel.clone(), timeout).await;
    // A progress storage failure must never skip stopping the owned engine.
    let progress_error = stage(shared, id, Stage::Stop).err();
    // Even a fully drained /voice response is not permission to leave our engine
    // resident or free the GPU while its process is still running.
    if let Err(mut error) = managed.finish(shared).await {
        if let Err(synthesis) = response {
            error
                .details
                .insert("synthesis_error".into(), synthesis.to_string());
        }
        return Err(error);
    }
    if let Some(error) = progress_error {
        return Err(error);
    }
    ensure_running(shared, id, &cancel)?;
    if response.is_ok() {
        stage(shared, id, Stage::Save)?;
    }
    response
}
async fn synthesize(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    managed: &Managed,
    binding: Binding,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<SpeechResponse, ApiError> {
    ensure_running(shared, id, &cancel)?;
    if managed.ready.process.has_exited().map_err(ApiError::io)? {
        return Err(ApiError::new(
            ErrorCode::EngineUnavailable,
            "SBV2が合成前に終了しました",
        ));
    }
    stage(shared, id, Stage::Synthesize)?;
    let prepared = managed.input.prepared().map_err(invalid)?;
    let client = Sbv2Client::new(managed.input.endpoint(), timeout).map_err(api_error)?;
    let exchange = client.exchange_prepared(prepared.audio(), &cancel);
    tokio::pin!(exchange);
    let bytes = loop {
        tokio::select! {
            result = &mut exchange => break result.map_err(|error| api_error(error.error))?,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Dropping an in-flight HTTP request does not settle the engine;
                // exchange() always stops the owned process before lease release.
                ensure_running(shared, id, &cancel)?;
                if managed.ready.process.has_exited().map_err(ApiError::io)? {
                    return Err(ApiError::new(ErrorCode::EngineUnavailable, "SBV2が合成中に終了しました"));
                }
            }
        }
    };
    ensure_running(shared, id, &cancel)?;
    stage(shared, id, Stage::Verify)?;
    let input = managed.input.clone();
    let catalog = managed.ready.catalog.clone();
    let settings = prepared.audio().settings().clone();
    let flag = cancel.clone();
    tokio::task::spawn_blocking(move || {
        binding.inspect_selected(
            input.endpoint(),
            &catalog,
            &settings,
            input.identity(),
            &flag,
        )
    })
    .await
    .map_err(invalid)??;
    ensure_running(shared, id, &cancel)?;
    SpeechResponse::sbv2(prepared.audio(), bytes).map_err(invalid)
}
fn ensure_running(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    cancel: &AtomicBool,
) -> Result<(), ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    let record = crate::speech_failure::load(&sessions, id)?;
    let (_, state) = crate::speech_journal::decode_input(&record)?;
    if cancel.load(Ordering::Acquire) || state.phase != JobPhase::Running {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "SBV2合成を取り消しました",
        ));
    }
    Ok(())
}
fn api_error(error: Error) -> ApiError {
    let code = match error {
        Error::Cancelled => ErrorCode::Cancelled,
        Error::Configuration => ErrorCode::ValidationFailed,
        Error::ModelChanged => ErrorCode::ExternalModification,
        _ => ErrorCode::EngineUnavailable,
    };
    ApiError::new(code, error.to_string())
}

fn stage(shared: &Arc<Mutex<Sessions>>, id: &str, stage: Stage) -> Result<(), ApiError> {
    let tracker = Tracker::default();
    tracker.set(stage);
    tracker.persist(shared, id)
}
