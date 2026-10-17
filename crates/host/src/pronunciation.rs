use crate::sessions::Sessions;
use std::sync::{Arc, Mutex};
use sungeki_protocol::*;

pub fn preview(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    expected_revision: u32,
    subtitle: String,
    spoken: Option<String>,
) -> Result<SessionResponse, ApiError> {
    let dictionary = {
        let sessions = shared
            .lock()
            .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?;
        let (_, state) = sessions.load(&session_id)?;
        if state.history().revision() != expected_revision {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "作品が更新されています。最新の辞書を確認してください",
            ));
        }
        state.history().project().pronunciation_dictionary.clone()
    };
    // Resolve against the captured dictionary outside the edit lock. The response
    // identifies its source revision even if another client edits in the meantime.
    let result = sungeki_speech::resolve_text(&subtitle, spoken.as_deref(), &dictionary)
        .map_err(|error| ApiError::new(ErrorCode::ValidationFailed, error.to_string()))?;
    Ok(SessionResponse::PronunciationPreview {
        session_id,
        revision: expected_revision,
        final_text: result.text,
        explicit_reading: result.source == sungeki_speech::TextSource::ExplicitReading,
        applied: result
            .applied
            .into_iter()
            .map(|used| {
                let entry = &dictionary[used.dictionary_index];
                PronunciationUse {
                    written: entry.written.clone(),
                    spoken: entry.spoken.clone(),
                    occurrences: used.occurrences as u32,
                }
            })
            .collect(),
    })
}
