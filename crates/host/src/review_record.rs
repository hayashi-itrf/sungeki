use serde::{Deserialize, Serialize};
use sungeki_project::{JobPhase, Progress, Project};
use sungeki_protocol::{ApiError, ErrorCode, ReviewReport};
use sungeki_storage::StoredJob;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    #[serde(default)]
    pub sampling: sungeki_protocol::ReviewFrameOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual: Option<VisualInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_unavailable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_job_id: Option<String>,
    pub source_revision: u32,
    pub content_hash: String,
    pub project: Project,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VisualInput {
    pub directory: std::path::PathBuf,
    pub runtime: sungeki_render::RenderRuntime,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub report: Option<ReviewReport>,
    pub error: Option<ApiError>,
}
impl State {
    pub fn queued() -> Self {
        Self {
            phase: JobPhase::Queued,
            progress: None,
            report: None,
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
    ApiError::new(ErrorCode::IoError, format!("ジョブ記録: {error:?}"))
}
pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != sungeki_storage::JobKind::Review {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "指定した確認ジョブが見つかりません",
        ));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    state.lifecycle()?;
    let hash = sungeki_storage::project_hash(&input.project).map_err(invalid)?;
    if input.parent_job_id.as_ref().is_some_and(|id| {
        uuid::Uuid::parse_str(id)
            .ok()
            .is_none_or(|u| u.to_string() != *id)
    }) {
        return Err(invalid("制作親IDが不正です"));
    }
    if hash != input.content_hash {
        return Err(invalid("入力のhashが一致しません"));
    }
    crate::review_worker::frames::validate(&input, job, &state)?;
    Ok((input, state))
}
