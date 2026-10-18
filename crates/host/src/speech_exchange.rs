//! One bounded HTTP exchange under durable dispatch ownership. The scheduler
//! must persist/verify the returned WAV before reporting job success.
use crate::{sessions::Sessions, speech_dispatch::Reservation, speech_state::invalid};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::{
    SpeechJobInput,
    voicevox::{TransportError, VoicevoxClient, VoicevoxDiscovery, VoicevoxResponse},
};

pub(crate) async fn run(
    shared: &Arc<Mutex<Sessions>>,
    job_id: &str,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<VoicevoxResponse, ApiError> {
    let reserved = Reservation::acquire(&mut *shared.lock().map_err(invalid)?, job_id)?;
    let input = reserved.input().voicevox()?.clone();
    crate::http_engine::ensure(shared, crate::http_engine::Engine::Voicevox, input.endpoint(), cancelled).await?;
    let prepared = input.prepared().map_err(invalid)?;
    let preflight = async {
        let client =
            VoicevoxClient::new(input.endpoint(), &input.identity().core_version, timeout)?;
        verify_engine(&input, cancelled, timeout).await?;
        if cancelled.load(Ordering::Acquire) {
            return Err(TransportError::Cancelled);
        }
        Ok(client)
    };
    let checked = match tokio::time::timeout(Duration::from_secs(15), preflight).await {
        Ok(result) => result,
        Err(_) => Err(TransportError::Timeout(
            sungeki_speech::voicevox::Stage::Catalog,
        )),
    };
    let client = match checked {
        Ok(client) => client,
        Err(error) => {
            let error = api_error(&error);
            crate::speech_failure::record(shared, job_id, error.clone(), cancelled)?;
            return Err(error); // reservation drop releases the still-unsent lease
        }
    };
    let begun = reserved.begin(&mut *shared.lock().map_err(invalid)?);
    let dispatched = match begun {
        Ok(dispatched) => dispatched,
        Err(error) if error.code == ErrorCode::Conflict => {
            let cancelled_error = api_error(&TransportError::Cancelled);
            crate::speech_failure::record(shared, job_id, cancelled_error.clone(), cancelled)?;
            return Err(cancelled_error);
        }
        Err(error) => return Err(error),
    };
    let result = client
        .synthesize_prepared(prepared.audio(), cancelled)
        .await;
    let unsettled = result
        .as_ref()
        .err()
        .is_some_and(TransportError::requires_engine_settlement);
    if unsettled {
        drop(dispatched); // preserve PossiblyRunning durably and quarantine in memory
    } else {
        dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
    }
    match result {
        Ok(response) if !cancelled.load(Ordering::Acquire) => {
            // Cancellation persisted while the final response was being drained
            // also wins, even if a caller has not yet set its wake-up flag.
            let sessions = shared.lock().map_err(invalid)?;
            let record = crate::speech_failure::load(&sessions, job_id)?;
            let (_, state) = crate::speech_journal::decode(&record)?;
            if state.phase == sungeki_project::JobPhase::Running {
                return Ok(response);
            }
            drop(sessions);
            let error = api_error(&TransportError::Cancelled);
            crate::speech_failure::record(shared, job_id, error.clone(), cancelled)?;
            Err(error)
        }
        result => {
            let error = api_error(&result.err().unwrap_or(TransportError::Cancelled));
            crate::speech_failure::record(shared, job_id, error.clone(), cancelled)?;
            Err(error)
        }
    }
}
async fn verify_engine(
    input: &SpeechJobInput,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<(), TransportError> {
    let discovery = VoicevoxDiscovery::new(input.endpoint(), timeout)?;
    let engine = discovery.engine(cancelled).await?;
    let expected = input.identity();
    if !engine.engine_id.eq_ignore_ascii_case(&expected.engine_id)
        || engine.version != expected.engine_version
    {
        return Err(TransportError::Metadata(
            "合成対象のエンジンが変更されています".into(),
        ));
    }
    let catalog = discovery
        .catalog(&engine, &expected.core_version, cancelled)
        .await?;
    let prepared = input
        .prepared()
        .map_err(|e| TransportError::Configuration(e.to_string()))?;
    if !catalog
        .speakers
        .iter()
        .flat_map(|s| &s.styles)
        .any(|style| style.id == prepared.audio().settings().style_id)
    {
        return Err(TransportError::Metadata(
            "合成対象の音声スタイルが利用できません".into(),
        ));
    }
    Ok(())
}
fn api_error(error: &TransportError) -> ApiError {
    let code = match error {
        TransportError::Cancelled => ErrorCode::Cancelled,
        TransportError::Configuration(_) | TransportError::Input(_) => ErrorCode::ValidationFailed,
        _ => ErrorCode::EngineUnavailable,
    };
    ApiError::new(code, error.to_string())
}
