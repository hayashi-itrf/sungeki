use crate::{build_state::invalid, sessions::Sessions};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

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
        .build_runtime
        .changed
        .clone();
    loop {
        // Use the same lock order as terminal commits; never retain Sessions while waiting.
        let (response, observed) = {
            let sessions = shared.lock().map_err(invalid)?;
            let generation = *signal.0.lock().map_err(invalid)?;
            (super::result(&sessions, id)?, generation)
        };
        let SessionResponse::BuildVideoJob { job } = &response else {
            unreachable!()
        };
        if job.phase.is_terminal() || Instant::now() >= deadline {
            return Ok(response);
        }
        let generation = signal.0.lock().map_err(invalid)?;
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
