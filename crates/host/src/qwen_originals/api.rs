//! Called by the shared host's blocking request worker, never on its async reactor.
use crate::{
    sessions::{Sessions, storage_error},
    speech_journal::{Input, decode_input},
    speech_state::invalid,
};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse, StartQwenSpeechArgs};

pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    args: StartQwenSpeechArgs,
) -> Result<SessionResponse, ApiError> {
    let (database, project, preparation) = {
        let sessions = shared.lock().map_err(invalid)?;
        if let Some(response) = replay(&sessions, &args)? {
            return Ok(response);
        }
        let (_, current) = sessions.load(&args.session_id)?;
        if current.history().revision() != args.expected_revision {
            return Err(conflict());
        }
        let runtime = sessions.speech_runtime.clone();
        if runtime.active.load(std::sync::atomic::Ordering::SeqCst) >= 8 {
            return Err(ApiError::new(
                ErrorCode::CapabilityUnavailable,
                "音声ジョブが混み合っています",
            ));
        }
        let key = serde_json::to_string(&args).map_err(invalid)?;
        let preparation = runtime
            .preparations
            .claim(&args.job_id, &args.session_id, &key)?;
        (
            sessions.review_database_path.clone(),
            current.history().project().clone(),
            preparation,
        )
    };
    let flag = AtomicBool::new(false);
    let profile = crate::qwen_originals::configuration::load(&database, &args.profile_id, &flag)?;
    let input = profile
        .capture(args.expected_revision, &project, &args.clip_id)?
        .for_profile(&args.profile_id)
        .map_err(invalid)?;
    // Runtime::start rechecks the revision/input when it creates the durable job.
    // Keep ownership continuous through directory preparation and durable commit.
    crate::speech_runtime::start_prepared(
        shared,
        args.session_id,
        args.job_id.clone(),
        &Input::Qwen(input),
        crate::speech_worker::Worker::Qwen {
            binding: profile.binding,
            port: profile.port,
        },
        Some(preparation),
    )?;
    crate::speech_api::result(&*shared.lock().map_err(invalid)?, &args.job_id)
}
fn replay(
    sessions: &Sessions,
    args: &StartQwenSpeechArgs,
) -> Result<Option<SessionResponse>, ApiError> {
    let Some(record) = sessions
        .store
        .load_job(&args.job_id)
        .map_err(storage_error)?
    else {
        return Ok(None);
    };
    if record.kind != sungeki_storage::JobKind::Speech {
        return Err(conflict());
    }
    let (input, _) = decode_input(&record)?;
    let Input::Qwen(input) = input else {
        return Err(conflict());
    };
    if record.session_id != args.session_id
        || input.source_revision() != args.expected_revision
        || input.clip_id() != args.clip_id
        || input.profile_id() != Some(args.profile_id.as_str())
    {
        return Err(conflict());
    }
    Ok(Some(crate::speech_api::result(sessions, &args.job_id)?))
}
fn conflict() -> ApiError {
    ApiError::new(
        ErrorCode::Conflict,
        "Qwen合成要求の対象・版・設定IDが一致しません。同じjob_idの入力は変更できません",
    )
}
