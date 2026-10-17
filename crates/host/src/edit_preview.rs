use crate::sessions::{Sessions, edit_error, preflight};
use sungeki_editor::{Command, EditAction, EditRequest, compare_projects};
use sungeki_protocol::*;
use sungeki_storage::project_hash;
impl Sessions {
    pub(crate) fn preview_edit(
        &self,
        id: String,
        revision: u32,
        commands: Vec<Command>,
        offset: u32,
        limit: u32,
    ) -> Result<SessionResponse, ApiError> {
        if !(1..=50).contains(&limit) {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "件数は1〜50です",
            ));
        }
        let (_, mut candidate) = self.load(&id)?;
        if candidate.history().revision() != revision {
            let mut error = ApiError::new(ErrorCode::Conflict, "確認対象が更新されています");
            error.details.insert(
                "current_revision".into(),
                candidate.history().revision().to_string(),
            );
            return Err(error);
        }
        let before = candidate.history().project().clone();
        // Run the same editor, including history and revision limits, but never commit this candidate.
        let receipt = candidate
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: revision,
                action: EditAction::Apply { commands },
            })
            .map_err(edit_error)?;
        preflight(&SessionResponse::Edited {
            receipt: receipt.clone(),
            snapshot: self.snapshot(id, &candidate)?,
        })?;
        let after = candidate.history().project();
        let entries = compare_projects(&before, after)
            .map_err(|e| ApiError::new(ErrorCode::ValidationFailed, e.to_string()))?;
        let total = u32::try_from(entries.len()).map_err(ApiError::io)?;
        if offset > total {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "差分ページが範囲外です",
            ));
        }
        let end = offset.saturating_add(limit).min(total);
        Ok(SessionResponse::EditPreview {
            preview: EditPreview {
                source_revision: revision,
                before_hash: project_hash(&before).map_err(crate::session_save::document_error)?,
                after_hash: project_hash(after).map_err(crate::session_save::document_error)?,
                before_duration_us: before.duration().unwrap(),
                after_duration_us: after.duration().unwrap(),
                no_op: receipt.no_op,
                created_ids: receipt.created_ids,
                total_changes: total,
                entries: entries
                    .into_iter()
                    .skip(offset as usize)
                    .take(limit as usize)
                    .collect(),
                next_offset: (end < total).then_some(end),
            },
        })
    }
}
