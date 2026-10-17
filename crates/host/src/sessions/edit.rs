use crate::sessions::{Sessions, edit_error, preflight, storage_error};
use sungeki_editor::EditRequest;
use sungeki_protocol::*;
use sungeki_storage::StoredSession;

impl Sessions {
    pub(crate) fn edit_session(
        &mut self,
        session_id: String,
        edit: EditRequest,
    ) -> Result<SessionResponse, ApiError> {
        let (stored, mut candidate) = self.load(&session_id)?;
        let receipt = candidate.execute(edit).map_err(|error| {
            let mut error = edit_error(error);
            error
                .details
                .insert("current_revision".into(), stored.revision.to_string());
            error
        })?;
        let mut response = SessionResponse::Edited {
            receipt: receipt.clone(),
            snapshot: self.snapshot(session_id.clone(), &candidate)?,
        };
        preflight(&response)?;
        let bytes = candidate.snapshot().map_err(edit_error)?;
        let committed = if bytes.as_slice() != stored.snapshot.as_slice() {
            let version = stored
                .version
                .checked_add(1)
                .ok_or_else(|| ApiError::new(ErrorCode::IoError, "保存世代の上限です"))?;
            let next = StoredSession {
                id: session_id.clone(),
                revision: candidate.history().revision(),
                version,
                snapshot: bytes.into(),
            };
            self.store
                .commit(Some(stored.version), &next)
                .map_err(storage_error)?;
            self.events.publish(
                &session_id,
                candidate.history().revision(),
                SessionEventKind::Edited,
                Some(receipt.operation_id.clone()),
            );
            next
        } else {
            stored
        };
        // The candidate and document binding cannot change while this request holds
        // the Sessions lock. Reuse the validated snapshot instead of cloning and
        // hashing the entire Project again after the durable write.
        if let SessionResponse::Edited { snapshot, .. } = &mut response {
            snapshot.event_cursor = self.events.cursor();
        }
        // Only durable, validated state enters the cache; errors leave the previous entry intact.
        self.parsed_cache.remember(committed, candidate);
        Ok(response)
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
