use crate::sessions::{Sessions, preflight, storage_error};
use std::time::{SystemTime, UNIX_EPOCH};
use sungeki_project::{Checkpoint, CheckpointAsset, CheckpointSummary};
use sungeki_protocol::*;
use sungeki_storage::{StoredCheckpoint, project_hash};

fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message)
}
impl Sessions {
    pub(crate) fn checkpoint_request(
        &mut self,
        id: String,
        request: CheckpointRequest,
    ) -> Result<SessionResponse, ApiError> {
        let (_, session) = self.load(&id)?;
        let project = session.history().project();
        match request {
            CheckpointRequest::Delete { checkpoint_id } => {
                let owner = match self
                    .store
                    .checkpoint(&checkpoint_id)
                    .map_err(storage_error)?
                {
                    Some(c) => Some(c.checkpoint.summary.project_id),
                    None => self
                        .store
                        .deleted_checkpoint_project(&checkpoint_id)
                        .map_err(storage_error)?,
                }
                .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "保存時点が見つかりません"))?;
                if owner != project.id {
                    return Err(invalid("別の作品の保存時点です"));
                }
                let response = SessionResponse::CheckpointDeleted {
                    checkpoint_id: checkpoint_id.clone(),
                };
                preflight(&response)?;
                if self
                    .store
                    .delete_checkpoint(&checkpoint_id, &project.id)
                    .map_err(storage_error)?
                {
                    self.events.publish(
                        &id,
                        session.history().revision(),
                        SessionEventKind::CheckpointDeleted,
                        None,
                    );
                }
                Ok(response)
            }
            CheckpointRequest::Compare {
                checkpoint_id,
                target,
                offset,
                limit,
            } => self.compare_checkpoint(
                project,
                session.history().revision(),
                checkpoint_id,
                target,
                offset,
                limit,
            ),
            CheckpointRequest::List { after, limit } => {
                if !(1..=50).contains(&limit) {
                    return Err(invalid("件数は1〜50です"));
                }
                let mut entries = self
                    .store
                    .list_checkpoints(&project.id, after.as_deref(), limit + 1)
                    .map_err(storage_error)?;
                let next_after = if entries.len() > limit as usize {
                    entries.pop();
                    entries.last().map(|c| c.id.clone())
                } else {
                    None
                };
                Ok(SessionResponse::Checkpoints {
                    checkpoints: entries,
                    next_after,
                })
            }
            CheckpointRequest::Create {
                checkpoint_id,
                name,
                expected_revision,
            } => {
                if uuid::Uuid::parse_str(&checkpoint_id)
                    .map_or(true, |id| id.hyphenated().to_string() != checkpoint_id)
                    || name.trim().is_empty()
                    || name.chars().count() > 200
                {
                    return Err(invalid("保存時点IDまたは名前が不正です"));
                }
                if self
                    .store
                    .deleted_checkpoint_project(&checkpoint_id)
                    .map_err(storage_error)?
                    .is_some()
                {
                    return Err(ApiError::new(
                        ErrorCode::Conflict,
                        "削除済みの保存時点IDは再利用できません",
                    ));
                }
                if let Some(existing) = self
                    .store
                    .checkpoint(&checkpoint_id)
                    .map_err(storage_error)?
                {
                    let c = existing.checkpoint.summary;
                    if existing.source_session_id == id
                        && c.project_id == project.id
                        && c.name == name
                        && c.source_revision == expected_revision
                    {
                        return Ok(SessionResponse::CheckpointCreated { checkpoint: c });
                    }
                    return Err(ApiError::new(
                        ErrorCode::Conflict,
                        "保存時点IDは使用済みです",
                    ));
                }
                if expected_revision != session.history().revision() {
                    return Err(ApiError::new(ErrorCode::Conflict, "文書が更新されています"));
                }
                let created_at = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(ApiError::io)?
                    .as_millis();
                let summary = CheckpointSummary {
                    id: checkpoint_id,
                    project_id: project.id.clone(),
                    name,
                    created_at: u64::try_from(created_at).map_err(ApiError::io)?,
                    source_revision: expected_revision,
                    content_hash: project_hash(project)
                        .map_err(crate::session_save::document_error)?,
                };
                let response = SessionResponse::CheckpointCreated {
                    checkpoint: summary.clone(),
                };
                preflight(&response)?;
                self.store
                    .create_checkpoint(&StoredCheckpoint {
                        source_session_id: id.clone(),
                        checkpoint: Checkpoint {
                            summary,
                            project: project.clone(),
                            asset_manifest: project
                                .assets
                                .iter()
                                .map(|a| CheckpointAsset {
                                    asset_id: a.id.clone(),
                                    locator: a.locator.clone(),
                                    content_hash: a.content_hash.clone(),
                                })
                                .collect(),
                        },
                    })
                    .map_err(storage_error)?;
                self.events.publish(
                    &id,
                    expected_revision,
                    SessionEventKind::CheckpointCreated,
                    None,
                );
                Ok(response)
            }
            CheckpointRequest::Restore {
                checkpoint_id,
                operation_id,
                expected_revision,
            } => {
                // Successful restores remain replayable even after checkpoint deletion.
                if let Some(record) = session.recorded_request(&operation_id) {
                    let matches = record.expected_revision == expected_revision
                        && matches!(&record.action, EditAction::Apply { commands }
                            if matches!(commands.as_slice(), [sungeki_editor::Command::ReplaceProject { checkpoint_id: Some(saved), .. }] if saved == &checkpoint_id));
                    if !matches {
                        return Err(ApiError::new(ErrorCode::Conflict, "操作IDは使用済みです"));
                    }
                    return self.request(SessionRequest::Edit {
                        session_id: id,
                        edit: record.clone(),
                    });
                }
                let checkpoint = self
                    .store
                    .checkpoint(&checkpoint_id)
                    .map_err(storage_error)?
                    .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "保存時点が見つかりません"))?
                    .checkpoint;
                if checkpoint.summary.project_id != project.id {
                    return Err(invalid("別の作品の保存時点です"));
                }
                self.request(SessionRequest::Edit {
                    session_id: id,
                    edit: EditRequest {
                        operation_id,
                        expected_revision,
                        action: EditAction::Apply {
                            commands: vec![sungeki_editor::Command::ReplaceProject {
                                checkpoint_id: Some(checkpoint_id),
                                project: checkpoint.project,
                            }],
                        },
                    },
                })
            }
        }
    }
}
