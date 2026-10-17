//! Shared host control; protocol/MCP/GUI mapping is a separate boundary.
use super::{
    Error, record,
    worker::{self, invalid, load},
};
use crate::sessions::Sessions;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::StoredJob;
fn reject(code: ErrorCode, message: &str) -> Error {
    Error::Api(ApiError::new(code, message))
}

pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    session_id: &str,
    job_id: &str,
    revision: u32,
    destination: &Path,
    dry_run: bool,
    include_checkpoints: bool,
) -> Result<StoredJob, Error> {
    if uuid::Uuid::parse_str(job_id)
        .ok()
        .is_none_or(|id| id.to_string() != job_id)
    {
        return Err(reject(
            ErrorCode::ValidationFailed,
            "job_idには標準UUIDが必要です",
        ));
    }
    if !destination.is_absolute() || destination.file_name().is_none() {
        return Err(reject(
            ErrorCode::ValidationFailed,
            "収集先には新しいフォルダの絶対パスが必要です",
        ));
    }
    let destination = destination
        .parent()
        .ok_or_else(|| invalid("親がありません"))?
        .canonicalize()
        .map_err(|e| {
            reject(
                ErrorCode::ValidationFailed,
                &format!("保存する場所を確認できません。選び直してください: {e}"),
            )
        })?
        .join(destination.file_name().unwrap());
    let mut sessions = shared.lock().map_err(invalid)?;
    if let Some(job) = sessions.store.load_job(job_id).map_err(invalid)? {
        let (input, _) = record::decode(&job)?;
        if job.session_id != session_id
            || input.source_revision != revision
            || input.destination != destination
            || input.dry_run != dry_run
            || input.include_checkpoints != include_checkpoints
        {
            return Err(reject(
                ErrorCode::Conflict,
                "同じjob_idに異なる要求は指定できません",
            ));
        }
        return Ok(job);
    }
    let (_, current) = sessions.load(session_id).map_err(Error::Api)?;
    if current.history().revision() != revision {
        return Err(reject(ErrorCode::Conflict, "作品が更新されています"));
    }
    let project = current.history().project().clone();
    let input = record::Input {
        source_revision: revision,
        dry_run,
        include_checkpoints,
        checkpoints: if include_checkpoints {
            super::freeze::capture(&sessions.store, &project.id)?
        } else {
            Vec::new()
        },
        content_hash: sungeki_storage::project_hash(&project)?,
        project,
        destination,
        collected_project_id: uuid::Uuid::new_v4().to_string(),
    };
    let runtime = sessions.render_runtime.clone();
    let mut flags = runtime.flags.lock().map_err(invalid)?;
    if runtime.stopping.load(Ordering::SeqCst) || runtime.active.load(Ordering::SeqCst) >= 4 {
        return Err(reject(
            ErrorCode::CapabilityUnavailable,
            "終了処理中、またはジョブが混み合っています",
        ));
    }
    let job = record::create(
        &mut sessions.store,
        job_id.into(),
        session_id.into(),
        &input,
    )?;
    let flag = Arc::new(AtomicBool::new(false));
    flags.insert(job.id.clone(), flag.clone());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(flags);
    drop(sessions);
    worker::launch(shared.clone(), runtime, job.id.clone(), flag);
    Ok(job)
}
pub(crate) fn cancel(shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<StoredJob, Error> {
    let mut sessions = shared.lock().map_err(invalid)?;
    let job = load(&sessions, id)?;
    let (_, mut state) = record::decode(&job)?;
    if !state.phase.is_terminal() {
        state.request_cancel();
        record::save_and_notify(&mut sessions, &job, &state)?;
        if let Some(flag) = sessions
            .render_runtime
            .flags
            .lock()
            .map_err(invalid)?
            .get(id)
        {
            flag.store(true, Ordering::SeqCst);
        }
    }
    load(&sessions, id)
}
#[cfg(test)]
#[path = "collection_control_tests.rs"]
mod tests;
