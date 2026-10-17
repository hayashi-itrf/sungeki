use crate::sessions::{Sessions, storage_error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sungeki_project::{JobPhase, Progress, Project};
use sungeki_protocol::{ApiError, ErrorCode, RenderOutput};
use sungeki_storage::{JobKind, StoredJob};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    #[serde(default)]
    pub purpose: sungeki_protocol::RenderPurpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_job_id: Option<String>,
    pub source_revision: u32,
    pub content_hash: String,
    pub project: Project,
    pub directory: PathBuf,
    pub runtime: sungeki_render::RenderRuntime,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    #[serde(default)]
    pub chunk_sha256: Vec<String>,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<RenderOutput>,
    pub error: Option<ApiError>,
}
impl State {
    pub fn queued() -> Self {
        Self {
            chunk_sha256: Vec::new(),
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
    ApiError::new(ErrorCode::IoError, format!("出力ジョブ: {error:?}"))
}
pub(crate) fn record(sessions: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    let job = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "出力ジョブが見つかりません"))?;
    if job.kind != JobKind::Render {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "出力ジョブが見つかりません",
        ));
    }
    Ok(job)
}
pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != JobKind::Render {
        return Err(invalid("ジョブ種類の不一致"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    state.lifecycle()?;
    if input.parent_job_id.is_some() && input.purpose != sungeki_protocol::RenderPurpose::Final {
        return Err(invalid("制作ジョブの子は正式出力に限ります"));
    }
    if input.parent_job_id.as_ref().is_some_and(|id| {
        uuid::Uuid::parse_str(id)
            .ok()
            .is_none_or(|u| u.to_string() != *id)
    }) {
        return Err(invalid("制作親IDが不正です"));
    }
    if sungeki_storage::project_hash(&input.project).map_err(invalid)? != input.content_hash
        || !input.directory.is_absolute()
        || (state.phase == JobPhase::Succeeded) != state.output.is_some()
    {
        return Err(invalid("入力または成功状態が不一致です"));
    }
    Ok((input, state))
}
pub(crate) fn save(
    sessions: &mut Sessions,
    job: &StoredJob,
    state: &State,
) -> Result<(), ApiError> {
    sessions
        .store
        .update_job(
            &job.id,
            JobKind::Render,
            job.version,
            &serde_json::to_vec(state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    sessions.render_runtime.notify_render_changed();
    Ok(())
}
pub(crate) fn recover(sessions: &mut Sessions) -> Result<(), ApiError> {
    let mut after = None;
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::Render), None, after.as_deref(), 100)
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

pub(crate) fn render_error(error: sungeki_render::RenderError) -> ApiError {
    if matches!(&error, sungeki_render::RenderError::SourceChanged(_)) {
        ApiError::new(ErrorCode::ExternalModification, error.to_string())
    } else if matches!(&error, sungeki_render::RenderError::Cancelled) {
        ApiError::new(ErrorCode::Cancelled, error.to_string())
    } else {
        ApiError::new(ErrorCode::IoError, format!("出力ジョブ: {error}"))
    }
}
