use crate::{
    session_save::document_error,
    sessions::{Sessions, edit_error, preflight, storage_error},
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_editor::EditSession;
use sungeki_protocol::*;
use sungeki_storage::{DocumentBinding, StoredSession, load_document, project_hash};

pub(crate) fn open(
    shared: &Arc<Mutex<Sessions>>,
    path: String,
    import_checkpoints: bool,
) -> Result<SessionResponse, ApiError> {
    // Reading and validation do not hold the edit lock. Publication is serialized below.
    let (document, checkpoints) = if import_checkpoints {
        let runtime = shared
            .lock()
            .map_err(|_| ApiError::io("ロック障害"))?
            .render_runtime
            .clone();
        let _slot = runtime.slots.clone().try_acquire_owned().map_err(|_| {
            ApiError::new(
                ErrorCode::CapabilityUnavailable,
                "他の素材処理が実行中です。終了後に取り込んでください",
            )
        })?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(55);
        let verified = crate::collection_execute::import::verify(Path::new(&path), || {
            runtime.stopping.load(std::sync::atomic::Ordering::SeqCst)
                || std::time::Instant::now() >= deadline
        })
        .map_err(crate::collection_execute::api::error)?;
        (verified.document, verified.checkpoints)
    } else {
        (
            load_document(Path::new(&path)).map_err(document_error)?,
            Vec::new(),
        )
    };
    let path = document.path.to_string_lossy().into_owned();
    let mut sessions = shared
        .lock()
        .map_err(|_| ApiError::new(ErrorCode::IoError, "ロック障害"))?;
    if import_checkpoints
        && sessions
            .render_runtime
            .stopping
            .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "終了処理中のため履歴取込を中止しました",
        ));
    }
    let owner = sessions
        .store
        .document_owner(&path, Some(&document.stamp))
        .map_err(storage_error)?;
    if sessions
        .store
        .save_intents()
        .map_err(storage_error)?
        .iter()
        .any(|intent| intent.path == path || owner.as_ref() == Some(&intent.session_id))
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "保存中または保存結果の確認待ちです",
        ));
    }
    if let Some(id) = owner {
        let binding = sessions
            .store
            .document_binding(&id)
            .map_err(storage_error)?
            .ok_or_else(|| ApiError::new(ErrorCode::IoError, "文書の保存情報がありません"))?;
        if binding.stamp != document.stamp {
            let mut error = ApiError::new(
                ErrorCode::ExternalModification,
                "ファイルが外部で変更されています。編集中の内容は保持しています",
            );
            error.details.insert("session_id".into(), id);
            return Err(error);
        }
        if binding.path != path
            && project_hash(&document.project).map_err(document_error)?
                != binding.saved_content_hash
        {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "同じ文書が異なる素材基準の場所で開かれています。元の編集状態は保持しました。別の文書コピーとして開いてください",
            ));
        }
        let (_, session) = sessions.load(&id)?;
        let response = SessionResponse::Snapshot {
            snapshot: sessions.snapshot(id.clone(), &session)?,
        };
        preflight(&response)?;
        if import_checkpoints {
            let outcome = sessions
                .store
                .import_checkpoints(&document.project.id, &id, &checkpoints)
                .map_err(storage_error)?;
            if outcome.inserted + outcome.relocated > 0 {
                sessions.events.publish(
                    &id,
                    session.history().revision(),
                    SessionEventKind::CheckpointCreated,
                    None,
                );
            }
        }
        return Ok(response);
    }
    let hash = project_hash(&document.project).map_err(document_error)?;
    let session = EditSession::new(document.project)
        .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let mut snapshot = sessions.snapshot(id.clone(), &session)?;
    snapshot.document_path = Some(path.clone());
    snapshot.dirty = false;
    let response = SessionResponse::Snapshot { snapshot };
    preflight(&response)?;
    let stored = StoredSession {
        id: id.clone(),
        revision: 0,
        version: 0,
        snapshot: session.snapshot().map_err(edit_error)?.into(),
    };
    let binding = DocumentBinding {
        session_id: id.clone(),
        path,
        stamp: document.stamp,
        saved_content_hash: hash,
    };
    if import_checkpoints {
        sessions
            .store
            .create_document_session_with_checkpoints(
                &stored,
                &binding,
                &session.history().project().id,
                &checkpoints,
            )
            .map_err(storage_error)?;
    } else {
        sessions
            .store
            .create_document_session(&stored, &binding)
            .map_err(storage_error)?;
    }
    sessions
        .events
        .publish(&id, 0, SessionEventKind::Created, None);
    Ok(SessionResponse::Snapshot {
        snapshot: sessions.snapshot(id, &session)?,
    })
}
