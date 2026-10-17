//! Durable collection state. Scheduling/API are separate; file I/O recovery must
//! run outside the editor lock and persist with the observed job version.
use super::{Error, receipt::Receipt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sungeki_project::{JobPhase, Project};
use sungeki_storage::{JobKind, RecoveryStore, StoredJob};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub source_revision: u32,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub include_checkpoints: bool,
    #[serde(default)]
    pub checkpoints: Vec<super::history::FrozenCheckpoint>,
    pub content_hash: String,
    pub project: Project,
    pub destination: PathBuf,
    pub collected_project_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    #[serde(default)]
    pub plan: Option<super::dry_run::Report>,
    #[serde(default)]
    pub stage: Option<sungeki_storage::CollectionStageRecord>,
    pub receipt: Option<Receipt>,
    pub error: Option<sungeki_protocol::ApiError>,
}
impl State {
    pub fn queued() -> Self {
        Self {
            phase: JobPhase::Queued,
            plan: None,
            stage: None,
            receipt: None,
            error: None,
        }
    }
    pub fn start(&mut self) -> Result<(), Error> {
        if self.phase != JobPhase::Queued {
            return Err(invalid("開始状態"));
        }
        self.phase = JobPhase::Running;
        Ok(())
    }
    pub fn stage_created(
        &mut self,
        stage: &sungeki_storage::CollectionStageRecord,
    ) -> Result<(), Error> {
        if self.phase != JobPhase::Running || self.stage.as_ref().is_some_and(|old| old != stage) {
            return Err(invalid("作業フォルダの記録状態が一致しません"));
        }
        self.stage = Some(stage.clone());
        Ok(())
    }
    pub fn request_cancel(&mut self) {
        if !self.phase.is_terminal() {
            self.phase = JobPhase::Cancelling;
        }
    }
    pub fn intent(&mut self, receipt: &Receipt) -> Result<(), Error> {
        if self.phase != JobPhase::Running {
            return Err(invalid("公開前に取消/終了済みです"));
        }
        if self.receipt.as_ref().is_some_and(|old| old != receipt) {
            return Err(invalid("公開記録の変更"));
        }
        self.receipt = Some(receipt.clone());
        Ok(())
    }
    /// Only after exclusive rename succeeded. A late cancel cannot undo a
    /// completed user-visible publication; unlike temporary render artifacts.
    pub fn published(&mut self, receipt: &Receipt) -> Result<(), Error> {
        if !matches!(self.phase, JobPhase::Running | JobPhase::Cancelling)
            || self.receipt.as_ref() != Some(receipt)
        {
            return Err(invalid("公開結果の不一致"));
        }
        self.phase = JobPhase::Succeeded;
        self.error = None;
        Ok(())
    }
    /// Call only after collection execution has returned and its stage guard
    /// finished cleanup. Uncertain publication must go through recovery instead.
    pub fn failed_before_publication(
        &mut self,
        cancelled: bool,
        error: sungeki_protocol::ApiError,
    ) -> Result<(), Error> {
        if self.phase.is_terminal() {
            return Err(invalid("終了状態の変更"));
        }
        self.phase = if cancelled {
            JobPhase::Cancelled
        } else {
            JobPhase::Failed
        };
        self.error = Some(error);
        Ok(())
    }
}
fn invalid(message: impl std::fmt::Debug) -> Error {
    Error::Invalid(format!("収集ジョブ: {message:?}"))
}
fn storage(e: sungeki_storage::StorageError) -> Error {
    invalid(e)
}
fn uuid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|v| v.to_string() == id)
}

pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), Error> {
    if job.kind != JobKind::Collection || !uuid(&job.id) {
        return Err(invalid("種類またはID"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    if sungeki_storage::project_hash(&input.project)? != input.content_hash
        || !input.destination.is_absolute()
        || input.destination.file_name().is_none()
        || !uuid(&input.collected_project_id)
        || input.collected_project_id == input.project.id
        || (state.phase == JobPhase::Succeeded
            && ((if input.dry_run {
                state.plan.is_none()
            } else {
                state.receipt.is_none()
            }) || state.error.is_some()))
        || (state.phase == JobPhase::Queued && (state.receipt.is_some() || state.plan.is_some()))
        || (input.dry_run && state.receipt.is_some())
        || (state.plan.is_some() && (!input.dry_run || state.phase != JobPhase::Succeeded))
    {
        return Err(invalid("入力または状態"));
    }
    if (!input.include_checkpoints && !input.checkpoints.is_empty()) {
        return Err(invalid("履歴指定"));
    }
    let prepared = super::history::Prepared::new(
        &input.project,
        &input.collected_project_id,
        &input.checkpoints,
    )?;
    prepared.byte_length()?;
    if let Some(stage) = &state.stage {
        stage.validate()?;
        if input.dry_run
            || state.phase == JobPhase::Queued
            || stage.destination() != input.destination
        {
            return Err(invalid("作業フォルダと入力の不一致"));
        }
    }
    if let Some(plan) = &state.plan {
        plan.validate(&input)?;
    }
    if let Some(receipt) = &state.receipt {
        receipt.validate()?;
        let expected = prepared.files();
        let observed: Vec<_> = receipt
            .files
            .iter()
            .filter(|file| file.path.starts_with("checkpoints/"))
            .cloned()
            .collect();
        if observed != expected {
            return Err(invalid("保存時点の公開記録と入力の不一致"));
        }
        if receipt.destination != input.destination
            || receipt.collected_project_id != input.collected_project_id
        {
            return Err(invalid("公開記録と入力の不一致"));
        }
    }
    Ok((input, state))
}
pub(crate) fn create(
    store: &mut RecoveryStore,
    id: String,
    session_id: String,
    input: &Input,
) -> Result<StoredJob, Error> {
    let job = StoredJob {
        id,
        session_id,
        kind: JobKind::Collection,
        version: 0,
        input: serde_json::to_vec(input).map_err(invalid)?,
        state: serde_json::to_vec(&State::queued()).map_err(invalid)?,
    };
    decode(&job)?;
    store.create_job(&job).map_err(storage)
}
pub(crate) fn save_and_notify(
    sessions: &mut crate::sessions::Sessions,
    observed: &StoredJob,
    state: &State,
) -> Result<(), Error> {
    save(&mut sessions.store, observed, state)?;
    sessions.render_runtime.notify_collection_changed();
    Ok(())
}
pub(crate) fn save(
    store: &mut RecoveryStore,
    observed: &StoredJob,
    state: &State,
) -> Result<(), Error> {
    let candidate = StoredJob {
        state: serde_json::to_vec(state).map_err(invalid)?,
        ..observed.clone()
    };
    decode(&candidate)?;
    store
        .update_job(
            &observed.id,
            JobKind::Collection,
            observed.version,
            &candidate.state,
        )
        .map_err(storage)?;
    Ok(())
}
/// Must receive a cancelled/deadline callback; scanning can read gigabytes.
/// Returns a candidate state, never holds a store/editor lock during the scan.
/// Persist via save with this exact observed version to reject concurrent writes.
pub(crate) fn recover(job: &StoredJob, cancelled: impl Fn() -> bool) -> Result<State, Error> {
    let (_, mut state) = decode(job)?;
    if state.phase.is_terminal() {
        return Ok(state);
    }
    if let Some(receipt) = &state.receipt {
        match receipt.verify(&cancelled) {
            Ok(()) => {
                state.phase = JobPhase::Succeeded;
                state.error = None;
                return Ok(state);
            }
            Err(e @ Error::Publish(sungeki_storage::PublishError::Cancelled)) => return Err(e),
            Err(e) if cancelled() => return Err(e),
            Err(e) => state.error = Some(super::errors::api(e)),
        }
    } else {
        state.error = Some(sungeki_protocol::ApiError::new(
            sungeki_protocol::ErrorCode::IoError,
            "公開前に中断しました。未完成フォルダは自動で再利用しません",
        ));
    }
    if let Some(stage) = &state.stage {
        let presence = stage.inspect()?;
        if let Some(error) = &mut state.error {
            error
                .details
                .insert("staging_path".into(), stage.path().to_string_lossy().into());
            error.details.insert(
                "staging_status".into(),
                format!("{presence:?}").to_lowercase(),
            );
        }
    }
    state.phase = JobPhase::Interrupted;
    Ok(state)
}
#[cfg(test)]
#[path = "collection_record_tests.rs"]
mod tests;
