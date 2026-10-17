use super::{
    Error, record,
    worker::{invalid, load},
};
use crate::sessions::Sessions;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    sync::{Arc, Mutex},
};
use sungeki_assets::{CollectionInput, CollectionLimits, plan_collection};
use sungeki_project::JobPhase;
use sungeki_protocol::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    pub summary: CollectionPlanSummary,
    pub items: Vec<CollectionPlanItem>,
}
pub(super) fn run(
    shared: &Arc<Mutex<Sessions>>,
    id: &str,
    input: &record::Input,
    cancelled: impl Fn() -> bool,
) -> Result<(), Error> {
    let result = inspect(input, &cancelled);
    let mut sessions = shared.lock().map_err(invalid)?;
    let job = load(&sessions, id)?;
    let (_, mut state) = record::decode(&job)?;
    if state.phase == JobPhase::Cancelling || cancelled() {
        state.failed_before_publication(
            true,
            super::errors::api(sungeki_storage::PublishError::Cancelled.into()),
        )?;
    } else {
        match result {
            Ok(report) => {
                state.plan = Some(report);
                state.phase = JobPhase::Succeeded;
            }
            Err(error) => {
                let api = super::errors::api(error);
                state.failed_before_publication(api.code == ErrorCode::Cancelled, api)?;
            }
        }
    }
    record::save_and_notify(&mut sessions, &job, &state)
}
fn inspect(input: &record::Input, cancelled: impl Fn() -> bool) -> Result<Report, Error> {
    let mut inputs = vec![CollectionInput {
        project: &input.project,
    }];
    inputs.extend(input.checkpoints.iter().map(|c| CollectionInput {
        project: &c.source.project,
    }));
    let plan =
        plan_collection(&inputs, CollectionLimits::default(), &cancelled).map_err(Error::Plan)?;
    let destination_exists = match fs::symlink_metadata(&input.destination) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    let mut items: Vec<Option<CollectionPlanItem>> = vec![None; plan.sources.len()];
    for file in &plan.files {
        for index in &file.source_indices {
            use sungeki_assets::CollectionSourceStatus as S;
            let source = &plan.sources[*index];
            let (status, detail) = match &source.status {
                S::Verified => (CollectionSourceStatus::Verified, None),
                S::Missing => (CollectionSourceStatus::Missing, None),
                S::Changed => (CollectionSourceStatus::Changed, None),
                S::NotInspected => (CollectionSourceStatus::NotInspected, None),
                S::Unresolved(d) => (
                    CollectionSourceStatus::Unresolved,
                    Some(d.chars().take(512).collect()),
                ),
                S::Unreadable(d) => (
                    CollectionSourceStatus::Unreadable,
                    Some(d.chars().take(512).collect()),
                ),
            };
            items[*index] = Some(CollectionPlanItem {
                asset_id: source.asset_id.clone(),
                checkpoint_id: source
                    .document_index
                    .checked_sub(1)
                    .map(|i| input.checkpoints[i].source.summary.id.clone()),
                source_path: source
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().chars().take(4096).collect()),
                source_path_truncated: source
                    .path
                    .as_ref()
                    .is_some_and(|p| p.to_string_lossy().chars().count() > 4096),
                destination_path: file.relative_path.clone(),
                sha256: file.sha256.clone(),
                byte_length: file.byte_length,
                status,
                detail,
                conflicting_metadata: file.conflicting_metadata,
            });
        }
    }
    if cancelled() {
        return Err(sungeki_storage::PublishError::Cancelled.into());
    }
    Ok(Report {
        summary: CollectionPlanSummary {
            checkpoint_count: input.checkpoints.len() as u32,
            sources_ready: plan.sources_ready(),
            destination_exists,
            source_count: items.len() as u32,
            unique_files: plan.files.len() as u32,
            estimated_asset_bytes: plan.estimated_asset_bytes,
            excluded_dependencies: [
                "tts_engines",
                "model_weights",
                "python",
                "node",
                "browser",
                "ffmpeg",
                "application_workers",
                "installed_fonts",
                "caches",
                "checkpoints",
            ]
            .into_iter()
            .filter(|name| *name != "checkpoints" || !input.include_checkpoints)
            .map(str::to_owned)
            .collect(),
        },
        items: items
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid("収集計画の参照が不足しています"))?,
    })
}
pub(super) fn page(
    shared: &Arc<Mutex<Sessions>>,
    args: ReadCollectionPlanArgs,
) -> Result<SessionResponse, ApiError> {
    if !(1..=100).contains(&args.limit) {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "limitは1〜100です",
        ));
    }
    let job = {
        let sessions = shared.lock().map_err(|e| ApiError::io(e.to_string()))?;
        load(&sessions, &args.job_id).map_err(super::errors::api)?
    };
    let (_, state) = record::decode(&job).map_err(super::errors::api)?;
    let report = state
        .plan
        .ok_or_else(|| ApiError::new(ErrorCode::Conflict, "完了した事前確認の結果が必要です"))?;
    if args.offset > report.summary.source_count {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "offsetが一覧の範囲外です",
        ));
    }
    Ok(SessionResponse::CollectionPlanPage {
        page: CollectionPlanPage {
            offset: args.offset,
            total: report.summary.source_count,
            items: report
                .items
                .into_iter()
                .skip(args.offset as usize)
                .take(args.limit as usize)
                .collect(),
        },
    })
}

impl Report {
    pub(super) fn validate(&self, input: &record::Input) -> Result<(), Error> {
        use std::collections::BTreeMap;
        let bad = || invalid("保存済み収集計画がsnapshotと一致しません");
        let assets: Vec<_> = input
            .project
            .assets
            .iter()
            .map(|a| (None, a))
            .chain(input.checkpoints.iter().flat_map(|c| {
                c.source
                    .project
                    .assets
                    .iter()
                    .map(move |a| (Some(&c.source.summary.id), a))
            }))
            .collect();
        if self.summary.checkpoint_count as usize != input.checkpoints.len()
            || self.items.len() != assets.len()
            || self.summary.source_count as usize != self.items.len()
        {
            return Err(bad());
        }
        let mut unique = BTreeMap::<&str, (u64, bool, bool)>::new();
        for (item, (checkpoint_id, asset)) in self.items.iter().zip(assets) {
            let bytes = match &asset.probe {
                sungeki_project::AssetProbe::Image(p) => p.byte_length,
                sungeki_project::AssetProbe::Audio(p) => p.byte_length,
                sungeki_project::AssetProbe::Psd(p) => p.byte_length,
                sungeki_project::AssetProbe::Video(p) => p.byte_length,
                sungeki_project::AssetProbe::Vrm(p) => p.byte_length,
            };
            if item.checkpoint_id.as_ref() != checkpoint_id
                || item.asset_id != asset.id
                || item.sha256 != asset.probe.sha256()
                || item.byte_length != bytes
                || ![
                    "png", "jpg", "webp", "wav", "mp3", "psd", "mov", "mkv", "vrm", "vrma",
                ]
                .iter()
                .any(|ext| item.destination_path == format!("assets/{}.{ext}", item.sha256))
            {
                return Err(bad());
            }
            let entry = unique.entry(&item.sha256).or_insert((bytes, false, false));
            entry.1 |= matches!(item.status, CollectionSourceStatus::Verified);
            entry.2 |= item.conflicting_metadata;
        }
        let bytes = unique
            .values()
            .try_fold(0u64, |sum, (n, _, _)| sum.checked_add(*n))
            .ok_or_else(bad)?;
        if self.summary.unique_files as usize != unique.len()
            || self.summary.estimated_asset_bytes != bytes
            || self.summary.sources_ready
                != unique
                    .values()
                    .all(|(_, verified, conflict)| *verified && !*conflict)
        {
            return Err(bad());
        }
        Ok(())
    }
}
