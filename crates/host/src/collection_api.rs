#[path = "collection_wait.rs"]
mod wait;
use super::{Error, control, record, worker};
use crate::sessions::{Sessions, storage_error};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use sungeki_project::JobPhase;
use sungeki_protocol::*;
use sungeki_storage::JobKind;
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    request: CollectionRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        CollectionRequest::Start { args } => {
            wait::validate(args.wait_ms.unwrap_or(0))?;
            control::start(
                shared,
                &args.session_id,
                &args.job_id,
                args.expected_revision,
                Path::new(&args.destination),
                args.dry_run,
                args.include_checkpoints,
            )
            .map_err(error)?;
            wait::get(shared, &args.job_id, args.wait_ms.unwrap_or(0))
        }
        CollectionRequest::ReadPlan { args } => super::dry_run::page(shared, args),
        CollectionRequest::Get { job_id, wait_ms } => wait::get(shared, &job_id, wait_ms),
        CollectionRequest::Cancel { job_id } => {
            control::cancel(shared, &job_id).map_err(error)?;
            result(shared, &job_id)
        }
        CollectionRequest::List {
            session_id,
            after,
            limit,
        } => {
            if !(1..=100).contains(&limit) {
                return Err(ApiError::new(
                    ErrorCode::ValidationFailed,
                    "limitは1〜100です",
                ));
            }
            let sessions = shared.lock().map_err(|e| error(worker::invalid(e)))?;
            Ok(SessionResponse::CollectionJobs {
                job_ids: sessions
                    .store
                    .list_jobs(
                        Some(JobKind::Collection),
                        session_id.as_deref(),
                        after.as_deref(),
                        limit,
                    )
                    .map_err(storage_error)?,
            })
        }
    }
}
fn result(shared: &Arc<Mutex<Sessions>>, id: &str) -> Result<SessionResponse, ApiError> {
    let sessions = shared.lock().map_err(|e| error(worker::invalid(e)))?;
    result_locked(&sessions, id)
}
fn result_locked(sessions: &Sessions, id: &str) -> Result<SessionResponse, ApiError> {
    let job = worker::load(sessions, id).map_err(error)?;
    let (input, state) = record::decode(&job).map_err(error)?;
    let (_, current) = sessions.load(&job.session_id)?;
    let stale = current.history().revision() != input.source_revision
        || sungeki_storage::project_hash(current.history().project())
            .map_err(crate::session_save::document_error)?
            != input.content_hash
        || !super::freeze::unchanged(&sessions.store, &input).map_err(error)?;
    let output = if state.phase == JobPhase::Succeeded {
        state.receipt.map(|r| CollectionOutput {
            project_path: r
                .destination
                .join("project.vvm.json")
                .to_string_lossy()
                .into(),
            manifest_path: r.destination.join("manifest.json").to_string_lossy().into(),
            directory: r.destination.to_string_lossy().into(),
            collected_project_id: r.collected_project_id,
            checkpoints_included: !input.checkpoints.is_empty(),
        })
    } else {
        None
    };
    Ok(SessionResponse::CollectionJob {
        job: CollectionJob {
            job_id: job.id,
            session_id: job.session_id,
            source_revision: input.source_revision,
            content_hash: input.content_hash,
            destination: input.destination.to_string_lossy().into(),
            phase: state.phase,
            dry_run: input.dry_run,
            include_checkpoints: input.include_checkpoints,
            checkpoint_count: input.checkpoints.len() as u32,
            plan: state.plan.map(|p| p.summary),
            output,
            stale,
            error: state.error,
        },
    })
}
pub(crate) fn error(e: Error) -> ApiError {
    super::errors::api(e)
}
