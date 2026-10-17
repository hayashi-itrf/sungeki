#[path = "build_api_profiles.rs"]
mod profiles;
#[path = "build_reuse_request.rs"]
mod reconcile;
#[path = "build_api_reuse.rs"]
mod reuse;
#[path = "build_reuse_source.rs"]
mod reuse_source;
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use sungeki_project::JobPhase;
use sungeki_protocol::*;
use sungeki_storage::JobKind;
#[path = "build_progress.rs"]
mod progress;
#[path = "build_wait.rs"]
mod wait;

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    request: BuildVideoRequest,
) -> Result<SessionResponse, ApiError> {
    let id = match &request {
        BuildVideoRequest::Start { args } => Some(args.job_id.clone()),
        _ => None,
    };
    execute_inner(shared, request).map_err(|mut error| {
        if let Some(id) = id
            && let Ok(sessions) = shared.lock()
            && let Ok(row) = sessions.store.load_job(&id)
        {
            error
                .details
                .insert("job_created".into(), row.is_some().to_string());
            error.details.insert("job_id".into(), id);
        }
        error
    })
}
fn execute_inner(
    shared: &Arc<Mutex<Sessions>>,
    request: BuildVideoRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        BuildVideoRequest::Start { args } => {
            let wait_ms = args.wait_ms.unwrap_or(0);
            wait::validate(wait_ms)?;
            args.recipe.validate().map_err(invalid)?;
            let engine = profiles::connection(&args)?;
            let continuation = reuse::capture(shared, &args)?;
            let bindings = profiles::capture(shared, &args)?;
            let row = build::create_continued(
                &mut *shared.lock().map_err(invalid)?,
                &args.job_id,
                args.recipe,
                Path::new(&args.project_path),
                engine,
                bindings,
                continuation,
            )?;
            if build::decode(&row)?.1.phase == JobPhase::Queued {
                build::launch(shared, &row.id, None).map_err(|mut e| {
                    e.details.insert("job_created".into(), "true".into());
                    e.details.insert("job_id".into(), row.id.clone());
                    e.suggested_actions
                        .push("同じ入力とjob_idで再送し、保存済みの制作を確認してください".into());
                    e
                })?;
            }
            wait::get(shared, &row.id, wait_ms)
        }
        BuildVideoRequest::ReconcileAudio { args } => {
            let id = args.job_id.clone();
            reconcile::execute(shared, args)?;
            result(&*shared.lock().map_err(invalid)?, &id)
        }
        BuildVideoRequest::ReadReuseSource { job_id } => Ok(SessionResponse::BuildReuseSource {
            source: reuse_source::read(&*shared.lock().map_err(invalid)?, &job_id)?,
        }),
        BuildVideoRequest::Get { job_id, wait_ms } => wait::get(shared, &job_id, wait_ms),
        BuildVideoRequest::Cancel { job_id } => {
            let row = shared
                .lock()
                .map_err(invalid)?
                .store
                .load_job(&job_id)
                .map_err(storage_error)?
                .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "制作が見つかりません"))?;
            build::cancel_owned(shared, &row)?;
            build::monitor_cancel(shared, &job_id, Duration::from_secs(30))?;
            result(&*shared.lock().map_err(invalid)?, &job_id)
        }
        BuildVideoRequest::List { after, limit } => Ok(SessionResponse::BuildVideoJobs {
            job_ids: shared
                .lock()
                .map_err(invalid)?
                .store
                .list_jobs(Some(JobKind::BuildVideo), None, after.as_deref(), limit)
                .map_err(storage_error)?,
        }),
    }
}
fn result(sessions: &Sessions, id: &str) -> Result<SessionResponse, ApiError> {
    let row = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "制作が見つかりません"))?;
    let (input, state) = build::decode(&row)?;
    let progress = Some(progress::display(&input, &state, sessions, &row)?);
    let (stored, current) = sessions.load(&row.session_id)?;
    let stale = has_outdated_output(&state)
        || stored.revision != state.expected_revision
        || current.history().project().id != input.plan.project_id
        || state.saved.as_ref().is_some_and(|s| {
            sungeki_storage::project_hash(current.history().project())
                .ok()
                .as_ref()
                != Some(&s.content_hash)
        });
    let review_asset_freshness = if let Some(output) = &state.output {
        Some(crate::review_jobs::result(sessions, &output.review_job_id)?.asset_freshness)
    } else {
        None
    };
    let output = state
        .output
        .map(|o| -> Result<_, ApiError> {
            Ok(BuildVideoOutput {
                project_path: o.project_path.to_string_lossy().into(),
                project_sha256: o.project_sha256,
                render_job_id: o.render_job_id,
                render: state
                    .rendered
                    .ok_or_else(|| invalid("出力記録がありません"))?,
                review_job_id: o.review_job_id,
                review: o.review,
            })
        })
        .transpose()?;
    Ok(SessionResponse::BuildVideoJob {
        job: BuildVideoJob {
            job_id: row.id,
            session_id: row.session_id,
            title: input.recipe.title,
            phase: state.phase,
            progress,
            completed_steps: state.receipts.len() as u32,
            total_steps: input.step_ids.len() as u32,
            child_job_id: state.child_id,
            source_revision: state.expected_revision,
            output,
            error: state.error,
            stale,
            review_asset_freshness,
        },
    })
}

pub(crate) fn has_outdated_output(state: &crate::build_state::State) -> bool {
    state
        .rendered
        .as_ref()
        .is_some_and(|output| output.layout_version != sungeki_render::CURRENT_LAYOUT_VERSION)
        || crate::review_jobs::has_outdated_images(
            state.output.as_ref().map(|output| &output.review),
        )
}
