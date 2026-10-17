use crate::sessions::{Sessions, storage_error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sungeki_project::{JobPhase, Progress, Project};
use sungeki_protocol::{ApiError, AudioPreviewOutput, ErrorCode};
use sungeki_storage::{JobKind, StoredJob};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub source_revision: u32,
    pub content_hash: String,
    pub project: Project,
    pub directory: PathBuf,
    pub runtime: sungeki_render::AudioRuntime,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<AudioPreviewOutput>,
    pub error: Option<ApiError>,
}
impl State {
    pub fn queued() -> Self {
        Self {
            phase: JobPhase::Queued,
            progress: None,
            output: None,
            error: None,
        }
    }
    pub fn lifecycle(&self) -> Result<sungeki_jobs::Lifecycle, ApiError> {
        sungeki_jobs::Lifecycle::restore(self.phase, self.progress.clone()).map_err(invalid)
    }
    pub fn update(&mut self, lifecycle: sungeki_jobs::Lifecycle) {
        self.phase = lifecycle.phase();
        self.progress = lifecycle.progress().cloned();
    }
}
pub(crate) fn invalid(error: impl std::fmt::Debug) -> ApiError {
    ApiError::new(ErrorCode::IoError, format!("確認音声ジョブ: {error:?}"))
}
pub(crate) fn record(sessions: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    let job = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "確認音声ジョブが見つかりません"))?;
    if job.kind != JobKind::AudioPreview {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "確認音声ジョブが見つかりません",
        ));
    }
    Ok(job)
}
pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != JobKind::AudioPreview {
        return Err(invalid("ジョブ種類の不一致"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    state.lifecycle()?;
    if sungeki_storage::project_hash(&input.project).map_err(invalid)? != input.content_hash
        || !input.directory.is_absolute()
        || (state.phase == JobPhase::Succeeded) != state.output.is_some()
    {
        return Err(invalid("入力または成功状態が不一致です"));
    }
    if let Some(output) = &state.output {
        let hash_ok = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        };
        if state.error.is_some()
            || output.byte_length == 0
            || output.byte_length > 512 * 1024 * 1024
            || output.sample_rate != 48000
            || output.sample_frames == 0
            || output.normalization != input.project.audio_mix.normalization
            || input.directory.join("output.mp4").to_str() != Some(output.path.as_str())
            || !hash_ok(&output.sha256)
            || output.chunk_sha256.len() as u64
                != output
                    .byte_length
                    .div_ceil(sungeki_render::AUDIO_PREVIEW_CHUNK as u64)
            || output.chunk_sha256.iter().any(|s| !hash_ok(s))
        {
            return Err(invalid("確認音声の成果物情報が不正です"));
        }
    }
    Ok((input, state))
}
pub(crate) fn save(
    sessions: &mut Sessions,
    job: &StoredJob,
    state: &State,
) -> Result<(), ApiError> {
    decode(&StoredJob {
        state: serde_json::to_vec(state).map_err(invalid)?,
        ..job.clone()
    })?;
    sessions
        .store
        .update_job(
            &job.id,
            JobKind::AudioPreview,
            job.version,
            &serde_json::to_vec(state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    sessions.render_runtime.notify_audio_changed();
    Ok(())
}
pub(crate) fn recover(sessions: &mut Sessions) -> Result<(), ApiError> {
    let mut after = None;
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::AudioPreview), None, after.as_deref(), 100)
            .map_err(storage_error)?;
        if ids.is_empty() {
            return Ok(());
        }
        for id in &ids {
            let job = record(sessions, id)?;
            let (_, mut state) = decode(&job)?;
            if !state.phase.is_terminal() {
                state.update(
                    sungeki_jobs::Lifecycle::recover(state.phase, state.progress.clone())
                        .map_err(invalid)?,
                );
                state.error = Some(invalid(
                    "hostが中断しました。作業領域は再利用せず保持しています。外部処理の残存確認は未実施です",
                ));
                save(sessions, &job, &state)?;
            }
        }
        after = ids.last().cloned();
    }
}

#[cfg(test)]
#[path = "audio_preview_tests.rs"]
mod tests;
