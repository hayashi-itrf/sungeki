use crate::sessions::Sessions;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sungeki_project::Project;
use sungeki_protocol::*;
use sungeki_storage::{DocumentBinding, DocumentError, FileStamp};

pub(crate) fn document_error(error: DocumentError) -> ApiError {
    let code = match error {
        DocumentError::ExternalModification => ErrorCode::ExternalModification,
        DocumentError::OutputExists => ErrorCode::OutputExists,
        DocumentError::Invalid(_) => ErrorCode::ValidationFailed,
        _ => ErrorCode::IoError,
    };
    ApiError::new(code, format!("文書保存: {error:?}"))
}
pub(crate) struct SavePlan {
    pub session_id: String,
    pub path: PathBuf,
    pub project: Project,
    pub expected: Option<FileStamp>,
    pub content_hash: String,
}
pub(crate) fn save(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    path: Option<String>,
    revision: u32,
) -> Result<SessionResponse, ApiError> {
    let plan = shared
        .lock()
        .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?
        .prepare_save(session_id, path, revision)?;
    // File I/O runs outside the session lock. Other clients may continue editing.
    let outcome = sungeki_storage::save_document(&plan.path, &plan.project, plan.expected.as_ref())
        .map_err(document_error);
    let mut sessions = shared
        .lock()
        .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?;
    sessions.release_save(&plan);
    let stamp = match outcome {
        Ok(stamp) => stamp,
        Err(error) => {
            // Validation/conflict happened before publication. Unknown I/O failures
            // retain the intent so restart can reconcile a possible completed rename.
            if error.code != ErrorCode::IoError {
                sessions
                    .store
                    .abandon_save(&plan.session_id)
                    .map_err(crate::sessions::storage_error)?;
            }
            return Err(error);
        }
    };
    let binding = DocumentBinding {
        session_id: plan.session_id.clone(),
        path: plan.path.to_string_lossy().into_owned(),
        stamp,
        saved_content_hash: plan.content_hash,
    };
    sessions.finish_save(binding)
}

pub(crate) fn recover_pending(sessions: &mut Sessions) -> Result<(), ApiError> {
    use sungeki_storage::{load_document, project_hash};
    for intent in sessions
        .store
        .save_intents()
        .map_err(crate::sessions::storage_error)?
    {
        match load_document(std::path::Path::new(&intent.path)) {
            Ok(document)
                if project_hash(&document.project).map_err(document_error)?
                    == intent.content_hash =>
            {
                sessions
                    .store
                    .bind_document(&DocumentBinding {
                        session_id: intent.session_id,
                        path: intent.path,
                        stamp: document.stamp,
                        saved_content_hash: intent.content_hash,
                    })
                    .map_err(crate::sessions::storage_error)?;
            }
            Err(DocumentError::Io(error)) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(document_error(DocumentError::Io(error)));
            }
            _ => sessions
                .store
                .abandon_save(&intent.session_id)
                .map_err(crate::sessions::storage_error)?,
        }
    }
    Ok(())
}
