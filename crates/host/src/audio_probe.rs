use crate::sessions::Sessions;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};
static PROBING: Mutex<()> = Mutex::new(());
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    path: String,
) -> Result<SessionResponse, ApiError> {
    let _permit = PROBING.try_lock().map_err(|_| {
        let mut error = ApiError::new(
            ErrorCode::Conflict,
            "別の音声を検査中です。再試行してください",
        );
        error.retryable = true;
        error
    })?;
    let (cancel, database) = {
        let sessions = shared.lock().map_err(ApiError::io)?;
        (
            sessions.render_runtime.stopping.clone(),
            sessions.review_database_path.clone(),
        )
    };
    if Path::new(&path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
    {
        let config = crate::audio_decode::Configuration::load(&database)?;
        return sungeki_media::probe_mp3(Path::new(&path), &config.runtime(), &cancel)
            .map(|probe| SessionResponse::AudioProbed { probe })
            .map_err(crate::audio_decode::error);
    }
    sungeki_media::probe_audio(Path::new(&path), &cancel)
        .map(|probe| SessionResponse::AudioProbed { probe })
        .map_err(crate::audio_analysis::error)
}
