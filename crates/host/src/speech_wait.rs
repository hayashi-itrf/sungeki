use crate::{sessions::Sessions, speech_state::invalid};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};
#[cfg(test)]
#[path = "speech_wait_tests.rs"]
mod tests;

pub(super) fn get(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    wait_ms: u32,
) -> Result<SessionResponse, ApiError> {
    validate(wait_ms)?;
    let deadline = Instant::now() + Duration::from_millis(wait_ms.into());
    let signal = shared
        .lock()
        .map_err(invalid)?
        .speech_runtime
        .changed
        .clone();
    loop {
        // Both readers and writers acquire Sessions before the generation mutex.
        // Release Sessions before waiting so edits/cancellation/workers can proceed.
        let (response, observed) = {
            let sessions = shared.lock().map_err(invalid)?;
            let generation = *signal.0.lock().map_err(invalid)?;
            (super::result(&sessions, id)?, generation)
        };
        let SessionResponse::SpeechJob { job } = &response else {
            unreachable!()
        };
        if job.phase.is_terminal() || Instant::now() >= deadline {
            return Ok(response);
        }
        let generation = signal.0.lock().map_err(invalid)?;
        // A change between inspection and subscription must not be missed.
        if *generation != observed {
            continue;
        }
        let _guard = signal
            .1
            .wait_timeout(
                generation,
                deadline.saturating_duration_since(Instant::now()),
            )
            .map_err(invalid)?;
    }
}

pub(super) fn validate(wait_ms: u32) -> Result<(), ApiError> {
    if wait_ms > 30_000 {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "待機は0〜30000msです",
        ));
    }
    Ok(())
}
