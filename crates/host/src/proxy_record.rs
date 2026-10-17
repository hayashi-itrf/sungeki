use crate::sessions::{Sessions, storage_error};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use sungeki_project::{AssetProbe, JobPhase, Progress, Project};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::{JobKind, StoredJob};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub source_revision: u32,
    pub content_hash: String,
    pub project: Project,
    pub asset_ids: Vec<String>,
    pub max_edge: u32,
    #[serde(default)]
    pub encoding: sungeki_media::video_probe::frames::proxy::ProxyEncoding,
    pub encoder_sha256: String,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub work_root: PathBuf,
    pub cache_root: PathBuf,
}
impl Input {
    pub fn key(&self, id: &str) -> Result<String, ApiError> {
        self.cache_key(id).map(|k| k.as_str().to_owned())
    }
    pub fn cache_key(&self, id: &str) -> Result<sungeki_assets::proxy::ProxyKey, ApiError> {
        let asset = self
            .project
            .assets
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| invalid("素材がありません"))?;
        let AssetProbe::Video(probe) = &asset.probe else {
            return Err(invalid("動画素材が必要です"));
        };
        let descriptor = sungeki_media::video_probe::frames::proxy::ProxySettings {
            max_edge: self.max_edge,
            encoding: self.encoding,
        }
        .descriptor()
        .map_err(invalid)?;
        let descriptor =
            serde_json::to_vec(&(descriptor, &self.encoder_sha256)).map_err(invalid)?;
        sungeki_assets::proxy::ProxyKey::new(probe, &descriptor).map_err(invalid)
    }
}
pub(crate) use sungeki_protocol::ProxyOutput as Output;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub completed: Vec<Output>,
    pub error: Option<ApiError>,
}
impl State {
    pub fn queued() -> Self {
        Self {
            phase: JobPhase::Queued,
            progress: None,
            completed: vec![],
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
    ApiError::new(ErrorCode::IoError, format!("軽量動画ジョブ: {error:?}"))
}
fn hash_ok(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != JobKind::Proxy {
        return Err(invalid("ジョブ種類の不一致"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    sungeki_project::validate(&input.project).map_err(invalid)?;
    state.lifecycle()?;
    if sungeki_storage::project_hash(&input.project).map_err(invalid)? != input.content_hash
        || input.asset_ids.is_empty()
        || input.asset_ids.len() > 64
        || input.asset_ids.iter().collect::<HashSet<_>>().len() != input.asset_ids.len()
        || !hash_ok(&input.encoder_sha256)
        || [
            &input.ffmpeg,
            &input.ffprobe,
            &input.work_root,
            &input.cache_root,
        ]
        .iter()
        .any(|p| !p.is_absolute())
        || state.completed.len() > input.asset_ids.len()
        || (state.phase == JobPhase::Queued && !state.completed.is_empty())
        || (state.phase == JobPhase::Succeeded
            && (state.completed.len() != input.asset_ids.len() || state.error.is_some()))
    {
        return Err(invalid("入力または進捗が不一致です"));
    }
    for id in &input.asset_ids {
        input.key(id)?;
    }
    let mut completed = HashSet::new();
    for output in &state.completed {
        if !completed.insert(&output.asset_id)
            || !input.asset_ids.contains(&output.asset_id)
            || output.key != input.key(&output.asset_id)?
            || !hash_ok(&output.sha256)
            || output.byte_length == 0
            || output.byte_length > sungeki_media::video_probe::frames::proxy::MAX_PROXY_BYTES
        {
            return Err(invalid("軽量動画の成果物記録が不正です"));
        }
    }
    Ok((input, state))
}
pub(crate) fn record(sessions: &Sessions, id: &str) -> Result<StoredJob, ApiError> {
    let job = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "軽量動画ジョブがありません"))?;
    if job.kind != JobKind::Proxy {
        return Err(ApiError::new(
            ErrorCode::NotFound,
            "軽量動画ジョブがありません",
        ));
    }
    Ok(job)
}
pub(crate) fn save(
    sessions: &mut Sessions,
    job: &StoredJob,
    state: &State,
) -> Result<(), ApiError> {
    let bytes = serde_json::to_vec(state).map_err(invalid)?;
    decode(&StoredJob {
        state: bytes.clone(),
        ..job.clone()
    })?;
    sessions
        .store
        .update_job(&job.id, JobKind::Proxy, job.version, &bytes)
        .map_err(storage_error)?;
    sessions.render_runtime.notify_proxy_changed();
    Ok(())
}
pub(crate) fn recover(sessions: &mut Sessions) -> Result<(), ApiError> {
    let mut after = None;
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::Proxy), None, after.as_deref(), 100)
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
                    "hostが中断しました。再実行には新しいジョブが必要です。作業領域の残存確認は未実施です",
                ));
                save(sessions, &job, &state)?;
            }
        }
        after = ids.last().cloned();
    }
}
#[cfg(test)]
#[path = "proxy_record_tests.rs"]
mod tests;
