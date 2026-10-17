//! Durable bookkeeping. Does not dispatch HTTP, remove artifacts, or mutate Project.
use crate::{
    sessions::{Sessions, storage_error},
    speech_state::{Execution, State, invalid},
};
use sungeki_jobs::Resources;
#[path = "speech_input.rs"]
mod input;
pub(crate) use input::Input;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::SpeechJobInput;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn decode(job: &StoredJob) -> Result<(SpeechJobInput, State), ApiError> {
    let (input, state) = decode_input(job)?;
    Ok((input.voicevox()?.clone(), state))
}
pub(crate) fn decode_input(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != JobKind::Speech {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "音声ジョブが見つかりません",
        ));
    }
    if job.input.len() > 16 * 1024 * 1024 || job.state.len() > 1024 * 1024 {
        return Err(invalid("記録が容量上限を超えています"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    state.validate()?;
    if state.execution == Execution::Reused && input.parent_job_id().is_none() {
        return Err(invalid("再利用音声には続行する親の記録が必要です"));
    }
    validate_input_timing(&input, &state)?;
    Ok((input, state))
}
/// Durable start after source-revision validation. Replays never requeue completed work.
pub(crate) fn create(
    sessions: &mut Sessions,
    job_id: String,
    session_id: String,
    input: &SpeechJobInput,
) -> Result<StoredJob, ApiError> {
    create_input(
        sessions,
        job_id,
        session_id,
        &Input::Voicevox(input.clone()),
    )
}
pub(crate) fn create_input(
    sessions: &mut Sessions,
    job_id: String,
    session_id: String,
    input: &Input,
) -> Result<StoredJob, ApiError> {
    if uuid::Uuid::parse_str(&job_id)
        .ok()
        .is_none_or(|id| id.to_string() != job_id)
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "job_idは標準形式のUUIDが必要です",
        ));
    }
    if sessions
        .store
        .load_job(&job_id)
        .map_err(storage_error)?
        .is_none()
    {
        input.validate_parent(sessions, &job_id, &session_id)?;
        let (_, current) = sessions.load(&session_id)?;
        if current.history().revision() != input.source_revision()
            || current.history().project() != input.project()
        {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "合成対象が更新されています",
            ));
        }
    }
    let record = StoredJob {
        id: job_id,
        session_id,
        kind: JobKind::Speech,
        version: 0,
        input: serde_json::to_vec(input).map_err(invalid)?,
        state: serde_json::to_vec(&State::queued()).map_err(invalid)?,
    };
    if record.input.len() > 16 * 1024 * 1024 {
        return Err(invalid("入力が16MiBを超えています"));
    }
    let result = sessions.store.create_job(&record).map_err(storage_error)?;
    decode_input(&result)?;
    Ok(result)
}
pub(crate) fn save(
    sessions: &mut Sessions,
    record: &StoredJob,
    state: &State,
) -> Result<(), ApiError> {
    if record.kind != JobKind::Speech {
        return Err(invalid("ジョブの種類が不正です"));
    }
    state.validate()?;
    let (input, old) = decode_input(record)?;
    validate_input_timing(&input, state)?;
    if matches!(
        state.application,
        crate::speech_state::Application::Applied { .. }
    ) && !matches!(
        old.application,
        crate::speech_state::Application::Applied { .. }
    ) {
        return Err(invalid(
            "音声の反映記録は作品と同じtransactionで保存してください",
        ));
    }
    crate::speech_transition::validate(&old, state)?;
    let bytes = encode(state)?;
    sessions
        .store
        .update_job(&record.id, JobKind::Speech, record.version, &bytes)
        .map_err(storage_error)?;
    sessions.speech_runtime.notify_changed();
    Ok(())
}
pub(crate) fn encode(state: &State) -> Result<Vec<u8>, ApiError> {
    state.validate()?;
    let bytes = serde_json::to_vec(state).map_err(invalid)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid("状態が1MiBを超えています"));
    }
    Ok(bytes)
}
pub(crate) fn recover(sessions: &mut Sessions) -> Result<Resources, ApiError> {
    let mut after = None;
    let mut unfinished = vec![];
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::Speech), None, after.as_deref(), 100)
            .map_err(storage_error)?;
        if ids.is_empty() {
            break;
        }
        for id in &ids {
            let record = sessions
                .store
                .load_job(id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("ジョブ欠落"))?;
            let (input, mut state) = decode_input(&record)?;
            if state.recover()? {
                save(sessions, &record, &state)?;
            }
            if state.execution == Execution::PossiblyRunning {
                // Conservatively serialize all aliases/ports for the same engine
                // UUID and retain the shared GPU slot until real settlement.
                unfinished.push((id.clone(), input.resources()));
                if unfinished.len() > 1024 {
                    return Err(invalid("未終了の合成が上限を超えています"));
                }
            }
        }
        after = ids.last().cloned();
    }
    Resources::recover(vec![], unfinished).map_err(invalid)
}

pub(crate) fn validate_input_timing(input: &Input, state: &State) -> Result<(), ApiError> {
    if let Some(timing) = state.artifact.as_ref().and_then(|a| a.lip_sync.as_ref()) {
        if timing.settings != input.lip_sync_settings() {
            return Err(invalid("口パク結果の設定が固定入力と一致しません"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "speech_sbv2_journal_tests.rs"]
mod sbv2_tests;

#[cfg(test)]
#[path = "speech_sbv2_output_tests.rs"]
mod sbv2_output_tests;

#[cfg(test)]
#[path = "speech_qwen_output_tests.rs"]
mod qwen_output_tests;
#[cfg(test)]
#[path = "speech_qwen_journal_tests.rs"]
mod qwen_tests;

#[cfg(test)]
#[path = "speech_irodori_output_tests.rs"]
mod irodori_output_tests;
#[cfg(test)]
#[path = "speech_irodori_journal_tests.rs"]
mod irodori_tests;

#[cfg(test)]
#[path = "build_reuse_target_tests.rs"]
mod reuse_target_tests;
