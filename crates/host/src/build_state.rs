//! Internal parent journal contract. Recipe decoding and dispatch are not yet
//! exposed; the execution plan is bound to the typed recipe and its digest.
#[path = "build_plan.rs"]
mod plan;
pub(crate) use plan::Plan;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::PathBuf};
use sungeki_jobs::{BuildSchedule, Lifecycle};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::{ApiError, ErrorCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<crate::build_record::continuation::Continuation>,
    pub recipe: sungeki_project::BuildRecipe,
    pub engine: Option<crate::build_record::SpeechConnection>,
    #[serde(default)]
    pub managed_profiles: crate::build_record::profiles::Bindings,
    pub document_path: PathBuf,
    pub plan: Plan,
    pub recipe_sha256: String,
    pub speech_count: u16,
    /// One fixed ID for each step, including editor operations and child jobs.
    pub step_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub id: String,
    pub revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    pub project_path: PathBuf,
    pub project_sha256: String,
    pub render_job_id: String,
    pub review_job_id: String,
    pub review: sungeki_protocol::ReviewReport,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preflight {
    pub revision: u32,
    pub content_hash: String,
    pub frame_count: u64,
    pub checked_sources: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BakeOutcome {
    NotRequired,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SavedProject {
    pub revision: u32,
    pub content_hash: String,
    pub stamp: sungeki_storage::FileStamp,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub receipts: Vec<Receipt>,
    pub child_id: Option<String>,
    pub expected_revision: u32,
    pub output: Option<Output>,
    pub preflight: Option<Preflight>,
    pub bake: Option<BakeOutcome>,
    pub saved: Option<SavedProject>,
    pub rendered: Option<sungeki_protocol::RenderOutput>,
    pub error: Option<ApiError>,
}
impl Input {
    pub fn new(
        recipe: sungeki_project::BuildRecipe,
        path: &std::path::Path,
    ) -> Result<Self, ApiError> {
        recipe.validate().map_err(invalid)?;
        let speech_count = recipe.scenes.iter().map(|s| s.lines.len()).sum::<usize>() as u16;
        let input = Self {
            continuation: None,
            engine: None,
            managed_profiles: Default::default(),
            document_path: sungeki_storage::document_location(path).map_err(invalid)?,
            recipe_sha256: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&recipe).map_err(invalid)?)
            ),
            plan: Plan::new(&recipe),
            recipe,
            speech_count,
            step_ids: (0..speech_count + 8)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect(),
        };
        input.plan.validate(&input.recipe, &input.step_ids)?;
        Ok(input)
    }
}
impl State {
    pub fn validate(&self, input: &Input) -> Result<(), ApiError> {
        BuildSchedule::new(input.speech_count).map_err(invalid)?;
        if let Some(source) = &input.continuation {
            source.validate(input.speech_count)?;
        }
        input.recipe.validate().map_err(invalid)?;
        crate::build_record::profiles::validate(&input.managed_profiles, &input.recipe)?;
        if let Some(engine) = &input.engine {
            engine.validate(&input.recipe)?;
        }
        if !input.document_path.is_absolute()
            || !input
                .document_path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.ends_with(".vvm.json"))
        {
            return Err(invalid("作品保存先が不正です"));
        }
        input.plan.validate(&input.recipe, &input.step_ids)?;
        let recipe_hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&input.recipe).map_err(invalid)?)
        );
        let speech_count: usize = input.recipe.scenes.iter().map(|s| s.lines.len()).sum();
        if recipe_hash != input.recipe_sha256 || speech_count != usize::from(input.speech_count) {
            return Err(invalid("制作入力と固定計画が一致しません"));
        }

        if !hash_ok(&input.recipe_sha256)
            || input.step_ids.len() != usize::from(input.speech_count) + 8
            || input
                .step_ids
                .iter()
                .any(|id| uuid::Uuid::parse_str(id).is_err())
            || input.step_ids.iter().collect::<HashSet<_>>().len() != input.step_ids.len()
            || self.receipts.len() > input.step_ids.len()
        {
            return Err(invalid("制作計画が不正です"));
        }
        let schedule = BuildSchedule::restore(
            input.speech_count,
            self.receipts.len() as u16,
            self.child_id.clone(),
        )
        .map_err(invalid)?;
        Lifecycle::restore(self.phase, self.progress.clone()).map_err(invalid)?;
        let mut revision = 0;
        for (receipt, expected) in self.receipts.iter().zip(&input.step_ids) {
            if &receipt.id != expected || receipt.revision < revision {
                return Err(invalid("完了工程の順序またはrevisionが不正です"));
            }
            revision = receipt.revision;
        }
        if self.expected_revision != revision
            || self
                .child_id
                .as_ref()
                .is_some_and(|id| input.step_ids.get(self.receipts.len()) != Some(id))
        {
            return Err(invalid("進行中の工程が計画と一致しません"));
        }
        if self.phase == JobPhase::Queued
            && (!self.receipts.is_empty() || self.child_id.is_some() || self.progress.is_some())
            || self.phase == JobPhase::Cancelled && self.child_id.is_some()
            || (self.phase == JobPhase::Succeeded) != schedule.step().is_none()
            || (self.phase == JobPhase::Succeeded) != self.output.is_some()
            || matches!(self.phase, JobPhase::Failed | JobPhase::Interrupted)
                != self.error.is_some()
        {
            return Err(invalid("制作工程とジョブ状態が一致しません"));
        }
        let validation_index = usize::from(input.speech_count) + 3;
        if (self.receipts.len() > validation_index) != self.preflight.is_some() {
            return Err(invalid("出力前検査の記録が工程と一致しません"));
        }
        if let Some(proof) = &self.preflight {
            if proof.revision != self.receipts[validation_index].revision
                || !hash_ok(&proof.content_hash)
                || !(1..=432_000).contains(&proof.frame_count)
                || proof.checked_sources == 0
                || proof.checked_sources
                    > input.recipe.assets.len() + usize::from(input.speech_count)
            {
                return Err(invalid("出力前検査の内容が不正です"));
            }
        }
        let bake_index = usize::from(input.speech_count) + 4;
        if (self.receipts.len() > bake_index) != self.bake.is_some() {
            return Err(invalid("事前描画の判定記録が工程と一致しません"));
        }
        if self.bake.is_some()
            && self.receipts[bake_index].revision != self.receipts[validation_index].revision
        {
            return Err(invalid("事前描画不要の判定と検査revisionが一致しません"));
        }
        let save_index = usize::from(input.speech_count) + 5;
        if (self.receipts.len() > save_index) != self.saved.is_some() {
            return Err(invalid("保存記録と工程が一致しません"));
        }
        if let Some(saved) = &self.saved {
            if saved.revision != self.receipts[save_index].revision
                || self.preflight.as_ref().is_none_or(|p| {
                    p.revision != saved.revision || p.content_hash != saved.content_hash
                })
                || !hash_ok(&saved.stamp.hash)
            {
                return Err(invalid("保存した作品と検査結果が一致しません"));
            }
        }
        let render_index = usize::from(input.speech_count) + 6;
        if (self.receipts.len() > render_index) != self.rendered.is_some() {
            return Err(invalid("動画出力の記録が工程と一致しません"));
        }
        if let Some(rendered) = &self.rendered {
            if self
                .saved
                .as_ref()
                .is_none_or(|s| s.revision != self.receipts[render_index].revision)
                || self
                    .preflight
                    .as_ref()
                    .is_none_or(|p| p.frame_count != rendered.frame_count)
                || !std::path::Path::new(&rendered.path).is_absolute()
                || !hash_ok(&rendered.sha256)
                || !hash_ok(&rendered.font_sha256)
                || rendered.byte_length == 0
                || rendered.layout_version == 0
            {
                return Err(invalid("動画出力の内容が不正です"));
            }
        }
        if let Some(output) = &self.output {
            let end = input.step_ids.len();
            if output.project_path != input.document_path
                || self
                    .saved
                    .as_ref()
                    .is_none_or(|s| s.stamp.hash != output.project_sha256)
                || !output.project_path.is_absolute()
                || !hash_ok(&output.project_sha256)
                || output.render_job_id != input.step_ids[end - 2]
                || output.review_job_id != input.step_ids[end - 1]
                || output.review.report_id != output.review_job_id
                || sungeki_project::Diagnostics::new(
                    output.review.diagnostics.checks.clone(),
                    output.review.diagnostics.issues.clone(),
                )
                .summary
                    != output.review.diagnostics.summary
                || self.saved.as_ref().is_none_or(|s| {
                    s.revision != output.review.source_revision
                        || s.content_hash != output.review.content_hash
                })
                || self.receipts[end - 1].revision != output.review.source_revision
            {
                return Err(invalid("制作成果物の参照が不正です"));
            }
        }
        Ok(())
    }
}
pub(crate) fn hash_ok(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn invalid(value: impl std::fmt::Debug) -> ApiError {
    ApiError::new(ErrorCode::IoError, format!("制作親ジョブ: {value:?}"))
}
