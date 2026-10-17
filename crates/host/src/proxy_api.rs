#[path = "proxy_remove.rs"]
mod remove;
#[path = "proxy_start.rs"]
mod start;
#[path = "proxy_wait.rs"]
mod wait;
use super::proxy_record::{decode, invalid, record, save};
use crate::sessions::{Sessions, storage_error};
use std::sync::{Arc, Mutex, atomic::Ordering};
use sungeki_protocol::*;
use sungeki_storage::JobKind;
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    request: ProxyRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        ProxyRequest::Remove { args } => remove::execute(shared, args),
        ProxyRequest::Start { mut args } => {
            let ms = args.wait_ms.take().unwrap_or(0);
            wait::validate(ms)?;
            let response = start::execute(shared, args)?;
            let SessionResponse::ProxyJob { job } = response else {
                unreachable!()
            };
            wait::get(shared, &job.job_id, ms)
        }
        ProxyRequest::Get { job_id, wait_ms } => wait::get(shared, &job_id, wait_ms),
        ProxyRequest::Cancel { job_id } => {
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
        ProxyRequest::List {
            session_id,
            after,
            limit,
        } => {
            let sessions = shared.lock().map_err(invalid)?;
            Ok(SessionResponse::ProxyJobs {
                job_ids: sessions
                    .store
                    .list_jobs(
                        Some(JobKind::Proxy),
                        session_id.as_deref(),
                        after.as_deref(),
                        limit,
                    )
                    .map_err(storage_error)?,
            })
        }
    }
}
pub(super) fn result(sessions: &Sessions, id: &str) -> Result<SessionResponse, ApiError> {
    let job = record(sessions, id)?;
    let (input, state) = decode(&job)?;
    let (_, current) = sessions.load(&job.session_id)?;
    let stale = input.source_revision != current.history().revision()
        || input.content_hash
            != sungeki_storage::project_hash(current.history().project()).map_err(invalid)?;
    Ok(SessionResponse::ProxyJob {
        job: ProxyJob {
            job_id: job.id,
            session_id: job.session_id,
            source_revision: input.source_revision,
            content_hash: input.content_hash,
            asset_ids: input.asset_ids,
            max_edge: input.max_edge,
            phase: state.phase,
            progress: state.progress,
            completed: state.completed,
            error: state.error,
            stale,
        },
    })
}
