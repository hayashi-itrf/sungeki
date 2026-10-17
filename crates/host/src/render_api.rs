#[path = "render_frame.rs"]
mod frame;
#[path = "render_read.rs"]
pub(crate) mod read;
#[path = "render_wait.rs"]
mod wait;
use crate::{
    render_record::{Input, State, decode, invalid, record, save},
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
    request: RenderJobRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        RenderJobRequest::Frame { args } => frame::execute(shared, args),
        RenderJobRequest::Preview { args } => {
            start_for(shared, args, None, RenderPurpose::EditingPreview)
        }
        RenderJobRequest::Read { args } => read::execute(shared, args),
        RenderJobRequest::Publish { args } => crate::render_publish::execute(shared, args),
        RenderJobRequest::Start { args } => start(shared, args),
        RenderJobRequest::Get { job_id, wait_ms } => wait::get(shared, &job_id, wait_ms),
        RenderJobRequest::Cancel { job_id } => {
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
        RenderJobRequest::List {
            session_id,
            after,
            limit,
        } => {
            let sessions = shared.lock().map_err(invalid)?;
            Ok(SessionResponse::RenderJobs {
                job_ids: sessions
                    .store
                    .list_jobs(
                        Some(JobKind::Render),
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
    Ok(SessionResponse::RenderJob {
        job: RenderJob {
            purpose: input.purpose,
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
pub(crate) fn plan(
    project: &sungeki_project::Project,
) -> Result<sungeki_render::RenderPlan, ApiError> {
    sungeki_render::RenderPlan::new(project, |id| check_speech(project, id))
        .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))
}
pub(crate) fn check_speech(
    project: &sungeki_project::Project,
    id: &str,
) -> Result<(), sungeki_render::RenderError> {
    match sungeki_speech::audio_reference_matches(project, id) {
        Ok(true) => Ok(()),
        _ => Err(sungeki_render::RenderError::Message(format!(
            "音声を再合成してください: {id}"
        ))),
    }
}

fn start(
    shared: &Arc<Mutex<Sessions>>,
    args: StartRenderArgs,
) -> Result<SessionResponse, ApiError> {
    start_bound(shared, args, None)
}
pub(crate) fn start_bound(
    shared: &Arc<Mutex<Sessions>>,
    args: StartRenderArgs,
    parent: Option<&StoredJob>,
) -> Result<SessionResponse, ApiError> {
    start_for(shared, args, parent, RenderPurpose::Final)
}
fn start_for(
    shared: &Arc<Mutex<Sessions>>,
    args: StartRenderArgs,
    parent: Option<&StoredJob>,
    purpose: RenderPurpose,
) -> Result<SessionResponse, ApiError> {
    let wait_ms = args.wait_ms.unwrap_or(0);
    wait::validate(wait_ms)?;
    let mut sessions = shared.lock().map_err(invalid)?;
    if let Some(parent) = parent {
        if sessions
            .store
            .load_job(&parent.id)
            .map_err(storage_error)?
            .as_ref()
            != Some(parent)
        {
            return Err(invalid("制作親が更新済みです"));
        }
        crate::build_record::validate_render_parent(
            &sessions,
            &parent.id,
            &args.job_id,
            &args.session_id,
            args.expected_revision,
        )?;
    }
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
        if job.kind != JobKind::Render {
            return Err(ApiError::new(ErrorCode::Conflict, "別の種類のjob_idです"));
        }
        let (input, _) = decode(&job)?;
        if job.session_id != args.session_id
            || input.purpose != purpose
            || input.source_revision != args.expected_revision
            || input.parent_job_id.as_deref() != parent.map(|p| p.id.as_str())
        {
            return Err(ApiError::new(ErrorCode::Conflict, "別の要求のjob_idです"));
        }
        drop(sessions);
        return wait::get(shared, &args.job_id, wait_ms);
    }
    if sessions.render_runtime.active.load(Ordering::SeqCst) >= 4 {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "出力ジョブが混み合っています",
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
    plan(&project)?;
    let runtime_config =
        crate::render_runtime::configuration(&sessions.review_database_path, &project)?;
    let root = crate::render_location::root(&sessions.review_database_path)
        .map_err(crate::render_location::unavailable)?;
    let directory = crate::render_location::path(&root, &args.job_id)?;
    let input = Input {
        purpose,
        parent_job_id: parent.map(|p| p.id.clone()),
        source_revision: args.expected_revision,
        content_hash: sungeki_storage::project_hash(&project).map_err(invalid)?,
        project,
        directory,
        runtime: runtime_config,
    };
    let job = StoredJob {
        id: args.job_id.clone(),
        kind: JobKind::Render,
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
    drop(sessions);
    crate::render_worker::launch(shared.clone(), runtime, job.id.clone(), flag);
    wait::get(shared, &job.id, wait_ms)
}
