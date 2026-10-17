#[path = "loudness_wait.rs"]
mod wait;
use crate::{
    loudness_record::{Input, State, decode, invalid, save},
    sessions::{Sessions, storage_error},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sungeki_project::{AssetLocator, AssetProbe, JobPhase};
use sungeki_protocol::*;
use sungeki_storage::{JobKind, StoredJob};
pub(crate) fn record(s: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    s.store
        .load_job(id)
        .map_err(storage_error)?
        .filter(|j| j.kind == JobKind::Loudness)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音量測定ジョブがありません"))
}
fn result(s: &Sessions, id: &str) -> Result<SessionResponse, ApiError> {
    let row = record(s, id)?;
    let (input, state) = decode(&row)?;
    let (_, current) = s.load(&row.session_id)?;
    let output = state
        .output
        .map(|v| {
            serde_json::to_value(v)
                .map_err(invalid)
                .and_then(|v| serde_json::from_value(v).map_err(invalid))
        })
        .transpose()?;
    Ok(SessionResponse::LoudnessJob {
        job: LoudnessJob {
            job_id: row.id,
            session_id: row.session_id,
            asset_id: input.asset_id,
            source_revision: input.source_revision,
            phase: state.phase,
            progress: state.progress,
            output,
            error: state.error,
            stale: current.history().revision() != input.source_revision,
        },
    })
}
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    request: LoudnessJobRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        LoudnessJobRequest::Start { args } => start(shared, args),
        LoudnessJobRequest::Get { job_id, wait_ms } => wait::get(shared, &job_id, wait_ms),
        LoudnessJobRequest::List {
            session_id,
            after,
            limit,
        } => {
            let s = shared.lock().map_err(invalid)?;
            Ok(SessionResponse::LoudnessJobs {
                job_ids: s
                    .store
                    .list_jobs(
                        Some(JobKind::Loudness),
                        session_id.as_deref(),
                        after.as_deref(),
                        limit,
                    )
                    .map_err(storage_error)?,
            })
        }
        LoudnessJobRequest::Cancel { job_id } => {
            let mut s = shared.lock().map_err(invalid)?;
            let row = record(&s, &job_id)?;
            let (_, mut state) = decode(&row)?;
            let mut life = state.lifecycle()?;
            if life.request_cancel() {
                state.phase = life.phase();
                state.progress = life.progress().cloned();
                save(&mut s, &row, &state)?;
                if let Some(flag) = s
                    .loudness_runtime
                    .flags
                    .lock()
                    .map_err(invalid)?
                    .get(&job_id)
                {
                    flag.store(true, Ordering::SeqCst);
                }
            }
            result(&s, &job_id)
        }
    }
}
fn start(
    shared: &Arc<Mutex<Sessions>>,
    args: StartLoudnessArgs,
) -> Result<SessionResponse, ApiError> {
    let wait_ms = args.wait_ms.unwrap_or(0);
    wait::validate(wait_ms)?;
    if uuid::Uuid::parse_str(&args.job_id)
        .ok()
        .is_none_or(|v| v.to_string() != args.job_id)
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "job_idには標準UUIDが必要です",
        ));
    }
    let mut s = shared.lock().map_err(invalid)?;
    if let Some(row) = s.store.load_job(&args.job_id).map_err(storage_error)? {
        if row.kind != JobKind::Loudness {
            return Err(ApiError::new(ErrorCode::Conflict, "別種類のjob_idです"));
        }
        let (input, _) = decode(&row)?;
        if row.session_id != args.session_id
            || input.source_revision != args.expected_revision
            || input.asset_id != args.asset_id
        {
            return Err(ApiError::new(ErrorCode::Conflict, "別の要求のjob_idです"));
        }
        drop(s);
        return wait::get(shared, &args.job_id, wait_ms);
    }
    let runtime = s.loudness_runtime.clone();
    if runtime.stopping.load(Ordering::SeqCst) || runtime.active.load(Ordering::SeqCst) >= 4 {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "測定が混み合っているかhostが終了中です",
        ));
    }
    let (_, current) = s.load(&args.session_id)?;
    if current.history().revision() != args.expected_revision {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "測定対象が更新されています",
        ));
    }
    let asset = current
        .history()
        .project()
        .assets
        .iter()
        .find(|a| a.id == args.asset_id)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音声素材がありません"))?;
    let (AssetProbe::Audio(probe), AssetLocator::Absolute { path }) =
        (&asset.probe, &asset.locator)
    else {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "絶対パスの登録済み音声を指定してください",
        ));
    };
    let saved = crate::runtime_settings::load(&s.review_database_path)?;
    let ffmpeg = crate::runtime_settings::file(
        std::env::var_os("SUNGEKI_RENDER_FFMPEG").map(PathBuf::from),
        saved.ffmpeg.as_deref(),
        "FFmpeg",
    )?;
    let input = Input {
        source_revision: args.expected_revision,
        asset_id: args.asset_id,
        source_path: path.into(),
        probe: probe.clone(),
        ffmpeg,
        work_root: crate::render_location::root(&s.review_database_path)?,
        analysis_version: sungeki_media::LOUDNESS_VERSION,
    };
    let row = StoredJob {
        id: args.job_id,
        session_id: args.session_id,
        kind: JobKind::Loudness,
        version: 0,
        input: serde_json::to_vec(&input).map_err(invalid)?,
        state: serde_json::to_vec(&State {
            phase: JobPhase::Queued,
            progress: None,
            output: None,
            error: None,
        })
        .map_err(invalid)?,
    };
    decode(&row)?;
    let flag = Arc::new(AtomicBool::new(false));
    let mut flags = runtime.flags.lock().map_err(invalid)?;
    if runtime.stopping.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "hostは終了処理中です",
        ));
    }
    s.store.create_job(&row).map_err(storage_error)?;
    flags.insert(row.id.clone(), flag.clone());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    drop(flags);
    drop(s);
    crate::loudness_worker::launch(shared.clone(), runtime, row.id.clone(), flag);
    wait::get(shared, &row.id, wait_ms)
}
