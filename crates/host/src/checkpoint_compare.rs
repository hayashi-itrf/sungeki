use crate::sessions::{Sessions, storage_error};
use sungeki_project::{Checkpoint, Project};
use sungeki_protocol::*;

impl Sessions {
    fn comparison_checkpoint(&self, id: &str, project_id: &str) -> Result<Checkpoint, ApiError> {
        let c = self
            .store
            .checkpoint(id)
            .map_err(storage_error)?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "保存時点が見つかりません"))?
            .checkpoint;
        if c.summary.project_id != project_id {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "別作品の保存時点です",
            ));
        }
        Ok(c)
    }
    pub(crate) fn compare_checkpoint(
        &self,
        current: &Project,
        revision: u32,
        id: String,
        target: CheckpointTarget,
        offset: u32,
        limit: u32,
    ) -> Result<SessionResponse, ApiError> {
        if !(1..=50).contains(&limit) {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "件数は1〜50です",
            ));
        }
        let before = self.comparison_checkpoint(&id, &current.id)?;
        let (after, target_checkpoint_id, target_revision) = match target {
            CheckpointTarget::Current { expected_revision } => {
                if expected_revision != revision {
                    return Err(ApiError::new(
                        ErrorCode::Conflict,
                        "比較対象が更新されています",
                    ));
                }
                (current.clone(), None, Some(revision))
            }
            CheckpointTarget::Checkpoint { checkpoint_id } => {
                let c = self.comparison_checkpoint(&checkpoint_id, &current.id)?;
                (c.project, Some(checkpoint_id), None)
            }
        };
        let entries = sungeki_editor::compare_projects(&before.project, &after)
            .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
        let total_changes = u32::try_from(entries.len()).map_err(ApiError::io)?;
        if offset > total_changes {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "比較ページが範囲外です",
            ));
        }
        let end = offset.saturating_add(limit).min(total_changes);
        let comparison = CheckpointComparison {
            checkpoint_id: id,
            target_checkpoint_id,
            target_revision,
            before_hash: before.summary.content_hash,
            after_hash: sungeki_storage::project_hash(&after)
                .map_err(crate::session_save::document_error)?,
            before_duration_us: before.project.duration().unwrap(),
            after_duration_us: after.duration().unwrap(),
            total_changes,
            entries: entries
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .collect(),
            next_offset: (end < total_changes).then_some(end),
        };
        Ok(SessionResponse::CheckpointCompared { comparison })
    }
}
