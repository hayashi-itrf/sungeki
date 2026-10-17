use super::result;
use crate::{
    review_record::{Input, State, decode, invalid},
    sessions::{Sessions, storage_error},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::*;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    job_id: String,
    expected_revision: u32,
    sampling: ReviewFrameOptions,
) -> Result<SessionResponse, ApiError> {
    start_bound(
        shared,
        session_id,
        job_id,
        expected_revision,
        None,
        sampling,
    )
}
pub(crate) fn start_bound(
    shared: &Arc<Mutex<Sessions>>,
    session_id: String,
    job_id: String,
    expected_revision: u32,
    parent: Option<&StoredJob>,
    sampling: ReviewFrameOptions,
) -> Result<SessionResponse, ApiError> {
    if sampling.text_safe_margin_percent.is_some_and(|v| v > 25) {
        return Err(invalid("安全余白は0〜25%で指定してください"));
    }
    if !(1..=120).contains(&sampling.max_frames) || sampling.requested_times_us.len() > 120 {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "代表画像は1〜120枚、指定時刻は120件以内です",
        ));
    }
    if uuid::Uuid::parse_str(&job_id)
        .ok()
        .is_none_or(|v| v.to_string() != job_id)
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "job_idには標準形式のUUIDが必要です",
        ));
    }
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
        crate::build_record::validate_review_parent(
            &sessions,
            &parent.id,
            &job_id,
            &session_id,
            expected_revision,
        )?;
    }
    if let Some(existing) = sessions.store.load_job(&job_id).map_err(storage_error)? {
        if existing.kind != JobKind::Review {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "job_idが別の種類のジョブで使われています",
            ));
        }
        let (input, _) = decode(&existing)?;
        if existing.session_id != session_id
            || input.source_revision != expected_revision
            || input.sampling != sampling
            || input.parent_job_id.as_deref() != parent.map(|p| p.id.as_str())
        {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "job_idが別の要求で使われています",
            ));
        }
        return Ok(SessionResponse::ReviewJob {
            job: result(&sessions, &job_id)?,
        });
    }
    if sessions.review_runtime.active.load(Ordering::SeqCst) >= 8 {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "確認ジョブが混み合っています。完了後に再試行してください",
        ));
    }
    sessions.review_directory = crate::review_workdir::root(&sessions.review_database_path);
    sessions
        .review_directory
        .as_ref()
        .map_err(crate::review_workdir::unavailable)?;
    let (_, current) = sessions.load(&session_id)?;
    if current.history().revision() != expected_revision {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "確認対象が更新されています",
        ));
    }
    let project = current.history().project().clone();
    if !sampling.requested_times_us.is_empty() {
        sungeki_review::plan_review_frames(
            &project,
            &sampling.requested_times_us,
            sampling.max_frames as usize,
        )
        .map_err(|error| ApiError::new(ErrorCode::ValidationFailed, error.0))?;
    }
    let (visual, visual_unavailable) =
        match crate::render_runtime::configuration(&sessions.review_database_path, &project) {
            Ok(runtime) => (
                Some(crate::review_record::VisualInput {
                    directory: sessions
                        .review_directory
                        .as_ref()
                        .map_err(Clone::clone)?
                        .join(format!("{job_id}.frames")),
                    runtime,
                }),
                None,
            ),
            Err(error) => (None, Some(error.message)),
        };
    let input = Input {
        sampling,
        visual,
        visual_unavailable,
        parent_job_id: parent.map(|p| p.id.clone()),
        source_revision: expected_revision,
        content_hash: sungeki_storage::project_hash(&project).map_err(invalid)?,
        project,
    };
    let initial = StoredJob {
        kind: JobKind::Review,
        id: job_id.clone(),
        session_id,
        input: serde_json::to_vec(&input).map_err(invalid)?,
        version: 0,
        state: serde_json::to_vec(&State::queued()).map_err(invalid)?,
    };
    sessions.store.create_job(&initial).map_err(storage_error)?;
    let response = SessionResponse::ReviewJob {
        job: result(&sessions, &job_id)?,
    };
    let runtime = sessions.review_runtime.clone();
    let cancellation = Arc::new(AtomicBool::new(false));
    runtime
        .cancellations
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(job_id.clone(), cancellation.clone());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(sessions);
    crate::review_worker::launch(shared.clone(), runtime, job_id, cancellation);
    Ok(response)
}
