#[path = "speech_wait.rs"]
mod wait;
use crate::{
    sessions::{Sessions, storage_error},
    speech_journal::decode,
    speech_state::{Application, Execution, invalid},
};
use std::sync::{Arc, Mutex};
use sungeki_project::{ClipContent, Voice};
use sungeki_protocol::*;
use sungeki_speech::{SpeechJobInput, VoicevoxIdentity};
#[cfg(test)]
#[path = "profile_shutdown_tests.rs"]
mod profile_shutdown_tests;

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    mut request: SpeechJobRequest,
) -> Result<SessionResponse, ApiError> {
    // Waiting affects the response, never preparation identity or synthesis input.
    let wait_ms = match &mut request {
        SpeechJobRequest::StartLocal { args } => args.wait_ms.take(),
        SpeechJobRequest::Start { args } => args.wait_ms.take(),
        SpeechJobRequest::StartSbv2 { args } => args.wait_ms.take(),
        SpeechJobRequest::StartQwen { args } => args.wait_ms.take(),
        SpeechJobRequest::StartIrodori { args } => args.wait_ms.take(),
        _ => None,
    };
    if let Some(ms) = wait_ms {
        wait::validate(ms)?;
    }
    let response = dispatch(shared, request)?;
    if let (Some(ms), SessionResponse::SpeechJob { job }) = (wait_ms, &response) {
        return wait::get(shared, &job.job_id, ms);
    }
    Ok(response)
}
fn dispatch(
    shared: &Arc<Mutex<Sessions>>,
    request: SpeechJobRequest,
) -> Result<SessionResponse, ApiError> {
    match request {
        SpeechJobRequest::StartLocal { args } => crate::local_speech::start(shared, args),
        SpeechJobRequest::ListIrodoriProfiles => crate::irodori_originals::profiles::list(shared),
        SpeechJobRequest::GetIrodoriProfile { args } => {
            crate::irodori_originals::profiles::get(shared, &args.profile_id)
        }
        SpeechJobRequest::StartIrodori { args } => {
            crate::irodori_originals::api::start(shared, args)
        }
        SpeechJobRequest::ListQwenProfiles => crate::qwen_originals::profiles::list(shared),
        SpeechJobRequest::GetQwenProfile { args } => {
            crate::qwen_originals::profiles::get(shared, &args.profile_id)
        }
        SpeechJobRequest::GetSbv2Profile { args } => {
            let (database, flag) = profile_inspection(shared)?;
            let profile =
                crate::sbv2_originals::configuration::load(&database, &args.profile_id, &flag)?;
            let details = crate::sbv2_originals::discovery::details(&profile, &flag)?;
            Ok(SessionResponse::Sbv2Profile { details })
        }
        SpeechJobRequest::ListSbv2Profiles => {
            let (database, flag) = profile_inspection(shared)?;
            let profiles = crate::sbv2_originals::configuration::summaries(&database, &flag)?;
            Ok(SessionResponse::Sbv2Profiles { profiles })
        }
        SpeechJobRequest::StartQwen { args } => crate::qwen_originals::api::start(shared, args),
        SpeechJobRequest::StartSbv2 { args } => crate::sbv2_api::start(shared, args),
        SpeechJobRequest::Start { args } => {
            let input = input(shared, &args)?;
            crate::speech_runtime::start(shared, args.session_id, args.job_id.clone(), &input)?;
            result(&*shared.lock().map_err(invalid)?, &args.job_id)
        }
        SpeechJobRequest::Get { job_id, wait_ms } => wait::get(shared, &job_id, wait_ms),
        SpeechJobRequest::Cancel { job_id } => {
            crate::speech_runtime::cancel(shared, &job_id)?;
            result(&*shared.lock().map_err(invalid)?, &job_id)
        }
        SpeechJobRequest::List {
            session_id,
            after,
            limit,
        } => {
            if !(1..=100).contains(&limit) {
                return Err(ApiError::new(
                    ErrorCode::ValidationFailed,
                    "件数は1〜100です",
                ));
            }
            let sessions = shared.lock().map_err(invalid)?;
            Ok(SessionResponse::SpeechJobs {
                job_ids: sessions
                    .store
                    .list_jobs(
                        Some(sungeki_storage::JobKind::Speech),
                        session_id.as_deref(),
                        after.as_deref(),
                        limit,
                    )
                    .map_err(storage_error)?,
            })
        }
    }
}
pub(crate) fn profile_inspection(
    shared: &Arc<Mutex<Sessions>>,
) -> Result<(std::path::PathBuf, Arc<std::sync::atomic::AtomicBool>), ApiError> {
    let sessions = shared.lock().map_err(invalid)?;
    let flag = sessions.profile_inspection_cancel.clone();
    if flag.load(std::sync::atomic::Ordering::Acquire) {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "ホスト終了中のため原本照合を取り消しました",
        ));
    }
    Ok((sessions.review_database_path.clone(), flag))
}
fn input(
    shared: &Arc<Mutex<Sessions>>,
    args: &StartSpeechArgs,
) -> Result<SpeechJobInput, ApiError> {
    let endpoint = sungeki_speech::voicevox::canonical_endpoint(&args.base_url)
        .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
    let sessions = shared.lock().map_err(invalid)?;
    if let Some(record) = sessions
        .store
        .load_job(&args.job_id)
        .map_err(storage_error)?
    {
        if record.kind != sungeki_storage::JobKind::Speech {
            return Err(conflict());
        }
        let (input, _) = decode(&record)?;
        if record.session_id != args.session_id
            || input.source_revision() != args.expected_revision
            || input.clip_id() != args.clip_id
            || input.endpoint() != endpoint
            || input.identity().engine_version != args.engine_version
        {
            return Err(conflict());
        }
        return Ok(input);
    }
    let (_, current) = sessions.load(&args.session_id)?;
    if current.history().revision() != args.expected_revision {
        return Err(conflict());
    }
    let project = current.history().project();
    let clip = project
        .clips
        .iter()
        .find(|clip| clip.id == args.clip_id)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音声クリップが見つかりません"))?;
    let ClipContent::Speech { character_id, .. } = &clip.content else {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "音声クリップを指定してください",
        ));
    };
    let character = project
        .characters
        .iter()
        .find(|c| &c.id == character_id)
        .ok_or_else(|| invalid("キャラクター欠落"))?;
    let Voice::Voicevox {
        engine_id,
        core_version,
        ..
    } = &character.voice
    else {
        return Err(ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "VOICEVOX用の開始要求です。キャラクターに対応する音声エンジンの開始操作を使ってください",
        ));
    };
    SpeechJobInput::new(
        args.expected_revision,
        project,
        &args.clip_id,
        &endpoint,
        &VoicevoxIdentity {
            engine_id: engine_id.clone(),
            core_version: core_version.clone(),
            engine_version: args.engine_version.clone(),
        },
    )
    .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))
}
fn conflict() -> ApiError {
    ApiError::new(
        ErrorCode::Conflict,
        "合成要求の対象・版・入力が一致しません。同じjob_idの入力は変更できません",
    )
}
pub(crate) fn result(sessions: &Sessions, id: &str) -> Result<SessionResponse, ApiError> {
    let record = crate::speech_failure::load(sessions, id)?;
    let (input, state) = crate::speech_journal::decode_input(&record)?;
    let (_, current) = sessions.load(&record.session_id)?;
    let prepared = input.application(current.history().project())?;
    let stale = if matches!(state.application, Application::Applied { .. }) {
        let project = current.history().project();
        let reference = project
            .clips
            .iter()
            .find(|c| c.id == input.clip_id())
            .and_then(|clip| {
                if let ClipContent::Speech { audio, .. } = &clip.content {
                    audio.as_ref()
                } else {
                    None
                }
            });
        !reference.is_some_and(|audio| {
            audio.input_fingerprint == prepared.fingerprint
                && audio.engine_version == prepared.engine_version
        }) || !input.applied_audio_matches(project)
    } else {
        !prepared.matches
    };
    let artifact = state.artifact.map(|a| SpeechArtifact {
        path: a.path,
        sha256: a.sha256,
        duration_us: a.duration_us,
        sample_rate: a.sample_rate,
        channels: a.channels,
        frames: a.frames,
        byte_len: a.byte_len,
        lip_sync: a.lip_sync,
    });
    let application = match state.application {
        Application::NotApplicable => SpeechApplication::NotApplicable,
        Application::PendingConflict => match input.parent_job_id() {
            Some(id) => SpeechApplication::PendingParent {
                parent_job_id: id.into(),
            },
            None => SpeechApplication::PendingConflict,
        },
        Application::Applied {
            revision,
            overlapping_clip_ids,
        } => SpeechApplication::Applied {
            revision,
            overlapping_clip_ids,
        },
    };
    Ok(SessionResponse::SpeechJob {
        job: SpeechJob {
            job_id: record.id,
            session_id: record.session_id,
            clip_id: input.clip_id().into(),
            source_revision: input.source_revision(),
            input_hash: input.input_hash().into(),
            phase: state.phase,
            progress: state.progress,
            engine_settlement_required: state.execution == Execution::PossiblyRunning,
            stale,
            artifact,
            application,
            error: state.error,
        },
    })
}
