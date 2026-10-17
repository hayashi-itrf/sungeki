use crate::{
    frame_preview_record::{Input, State, decode, invalid, record, save},
    sessions::{Sessions, storage_error},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::*;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    request: FramePreviewRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        FramePreviewRequest::Read { args } => crate::frame_preview_read::execute(shared, args),
        FramePreviewRequest::Publish { args } => {
            crate::frame_preview_publish::execute(shared, args)
        }
        FramePreviewRequest::Start { args } => start(shared, args),
        FramePreviewRequest::Get { job_id } => result(&*shared.lock().map_err(invalid)?, &job_id),
        FramePreviewRequest::Cancel { job_id } => {
            let mut sessions = shared.lock().map_err(invalid)?;
            let job = record(&sessions, &job_id)?;
            let (_, mut state) = decode(&job)?;
            let mut lifecycle = state.lifecycle()?;
            if lifecycle.request_cancel() {
                state.update(lifecycle);
                save(&mut sessions, &job, &state)?;
                if let Some(flag) = sessions
                    .render_runtime
                    .flags
                    .lock()
                    .map_err(invalid)?
                    .get(&job_id)
                {
                    flag.store(true, Ordering::SeqCst);
                }
            }
            result(&sessions, &job_id)
        }
        FramePreviewRequest::List {
            session_id,
            after,
            limit,
        } => {
            let sessions = shared.lock().map_err(invalid)?;
            Ok(SessionResponse::FramePreviewJobs {
                job_ids: sessions
                    .store
                    .list_jobs(
                        Some(JobKind::FramePreview),
                        session_id.as_deref(),
                        after.as_deref(),
                        limit,
                    )
                    .map_err(storage_error)?,
            })
        }
    }
}
fn result(sessions: &Sessions, id: &str) -> Result<SessionResponse, ApiError> {
    let job = record(sessions, id)?;
    let (input, state) = decode(&job)?;
    let (_, current) = sessions.load(&job.session_id)?;
    let stale = current.history().revision() != input.source_revision
        || sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
            != input.content_hash
        || state
            .output
            .as_ref()
            .is_some_and(|output| output.layout_version != sungeki_render::CURRENT_LAYOUT_VERSION);
    Ok(SessionResponse::FramePreviewJob {
        job: FramePreviewJob {
            job_id: job.id,
            session_id: job.session_id,
            source_revision: input.source_revision,
            content_hash: input.content_hash,
            phase: state.phase,
            progress: state.progress,
            output: state.output,
            error: state.error,
            stale,
        },
    })
}
fn start(
    shared: &Arc<Mutex<Sessions>>,
    args: StartFramePreviewArgs,
) -> Result<SessionResponse, ApiError> {
    let mut sessions = shared.lock().map_err(invalid)?;
    if uuid::Uuid::parse_str(&args.job_id)
        .ok()
        .is_none_or(|v| v.to_string() != args.job_id)
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "job_idには標準UUIDが必要です",
        ));
    }
    if let Some(job) = sessions
        .store
        .load_job(&args.job_id)
        .map_err(storage_error)?
    {
        if job.kind != JobKind::FramePreview {
            return Err(ApiError::new(ErrorCode::Conflict, "別の種類のjob_idです"));
        }
        let (input, _) = decode(&job)?;
        if job.session_id != args.session_id
            || input.source_revision != args.expected_revision
            || input.frame_index != args.frame_index
        {
            return Err(ApiError::new(ErrorCode::Conflict, "別の要求のjob_idです"));
        }
        return result(&sessions, &args.job_id);
    }
    if sessions.render_runtime.active.load(Ordering::SeqCst) >= 4 {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "確認画像ジョブが混み合っています",
        ));
    }
    let (_, current) = sessions.load(&args.session_id)?;
    if current.history().revision() != args.expected_revision {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "出力対象が更新されています",
        ));
    }
    let project = current.history().project().clone();
    sungeki_render::FramePlan::new(&project, args.frame_index)
        .map_err(|error| ApiError::new(ErrorCode::ValidationFailed, error.to_string()))?;
    let runtime_config =
        crate::render_runtime::configuration(&sessions.review_database_path, &project)?;
    let root = crate::render_location::root(&sessions.review_database_path)?;
    let directory = crate::render_location::path(&root, &args.job_id)?;
    let input = Input {
        frame_index: args.frame_index,
        source_revision: args.expected_revision,
        content_hash: sungeki_storage::project_hash(&project).map_err(invalid)?,
        project,
        directory,
        runtime: runtime_config,
    };
    let job = StoredJob {
        id: args.job_id.clone(),
        kind: JobKind::FramePreview,
        session_id: args.session_id,
        input: serde_json::to_vec(&input).map_err(invalid)?,
        version: 0,
        state: serde_json::to_vec(&State::queued()).map_err(invalid)?,
    };
    let runtime = sessions.render_runtime.clone();
    let flag = Arc::new(AtomicBool::new(false));
    let mut flags = runtime.flags.lock().map_err(invalid)?;
    if runtime.stopping.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "hostは終了処理中です",
        ));
    }
    sessions.store.create_job(&job).map_err(storage_error)?;
    flags.insert(job.id.clone(), flag.clone());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(flags);
    let response = result(&sessions, &job.id);
    drop(sessions);
    crate::frame_preview_worker::launch(shared.clone(), runtime, job.id, flag);
    response
}
