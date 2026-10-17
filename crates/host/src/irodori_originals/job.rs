//! Durable reservation precedes model loading; settlement precedes publication.
use super::{binding::Binding, exchange, startup::Request};
use crate::{
    sessions::Sessions, speech_dispatch::Reservation, speech_journal::Input, speech_state::invalid,
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
use sungeki_speech::SpeechResponse;
pub(crate) async fn run(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    binding: Binding,
    port: u16,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(), ApiError> {
    let result = execute(shared, id, binding, port, cancel.clone(), timeout).await;
    match result {
        Ok(response) => crate::speech_output::complete(shared, id, response, &cancel).await,
        Err(error) => {
            crate::speech_failure::record(shared, id, error.clone(), &cancel)?;
            Err(error)
        }
    }
}
async fn execute(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    binding: Binding,
    port: u16,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<SpeechResponse, ApiError> {
    let reservation = Reservation::acquire(&mut *shared.lock().map_err(invalid)?, id)?;
    let Input::Irodori(input) = reservation.input() else {
        return Err(invalid("Irodori専用の実行経路です"));
    };
    let input = input.clone();
    let prepared = input.prepared().map_err(invalid)?;
    let progress = crate::speech_state::progress::Tracker::default();
    let request = Request {
        progress: progress.clone(),
        binding,
        port,
        selection: prepared.audio().audio_source().clone(),
        settings: prepared.audio().audio_source().settings.clone(),
        identity: input.identity().clone(),
    };
    let dispatched = reservation.begin(&mut *shared.lock().map_err(invalid)?)?;
    if let Err(error) = progress.persist(shared, id) {
        // No engine has been launched yet; release the recorded reservation.
        dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
        return Err(error);
    }
    let exchange = exchange::run(request, prepared.audio().clone(), cancel.clone(), timeout);
    tokio::pin!(exchange);
    let mut observation_error = None;
    let result = loop {
        tokio::select! {
            result=&mut exchange=>break result,
            _=tokio::time::sleep(Duration::from_millis(100))=> {
                if let Err(error)=running(shared,id,&cancel).and_then(|_| progress.persist(shared, id)) {cancel.store(true,Ordering::Release);observation_error.get_or_insert(error);}
            }
        }
    };
    match result {
        Ok(audio) => {
            dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
            if let Some(error) = observation_error {
                return Err(error);
            }
            running(shared, id, &cancel)?;
            progress.set(crate::speech_state::progress::Stage::Save);
            progress.persist(shared, id)?;
            SpeechResponse::irodori(prepared.audio(), audio.bytes().to_vec()).map_err(invalid)
        }
        Err(failure) => {
            if failure.cleanup_confirmed {
                dispatched.confirm_finished(&mut *shared.lock().map_err(invalid)?)?;
            } else {
                drop(dispatched);
            }
            let mut error = observation_error.unwrap_or(failure.error);
            error.details.insert(
                "cleanup_confirmed".into(),
                failure.cleanup_confirmed.to_string(),
            );
            if let Some(cleanup) = failure.cleanup_error {
                error
                    .details
                    .insert("cleanup_error".into(), cleanup.to_string());
            }
            // Unsettled process drops only after retaining the durable quarantine.
            Err(error)
        }
    }
}
fn running(shared: &Arc<Mutex<Sessions>>, id: &str, cancel: &AtomicBool) -> Result<(), ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    let record = crate::speech_failure::load(&sessions, id)?;
    let (_, state) = crate::speech_journal::decode_input(&record)?;
    if cancel.load(Ordering::Acquire) || state.phase != JobPhase::Running {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "Irodori合成を取り消しました",
        ));
    }
    Ok(())
}
