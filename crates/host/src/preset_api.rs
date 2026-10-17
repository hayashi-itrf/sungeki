use crate::sessions::Sessions;
use std::sync::{Arc, Mutex};
use sungeki_project::PresetContent;
use sungeki_protocol::*;
use sungeki_storage::{PresetLibrary, StorageError, StoredPreset};

fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
fn storage(error: StorageError) -> ApiError {
    let code = match error {
        StorageError::Conflict => ErrorCode::Conflict,
        StorageError::InvalidVersion | StorageError::InvalidSnapshot => ErrorCode::ValidationFailed,
        _ => ErrorCode::IoError,
    };
    ApiError::new(code, format!("プリセット保存: {error:?}"))
}
fn id(value: &str) -> Result<(), ApiError> {
    if uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value) {
        Ok(())
    } else {
        Err(invalid("プリセットIDは標準形式のUUIDで指定してください"))
    }
}
fn record(stored: StoredPreset) -> Result<PresetRecord, ApiError> {
    let preset = stored
        .payload
        .as_deref()
        .map(|bytes| {
            let preset: sungeki_project::Preset =
                serde_json::from_slice(bytes).map_err(ApiError::io)?;
            preset
                .validate()
                .map_err(|e| ApiError::new(ErrorCode::IoError, e.0))?;
            Ok::<_, ApiError>(preset)
        })
        .transpose()?;
    Ok(PresetRecord {
        id: stored.id,
        revision: stored.revision,
        preset,
    })
}

pub fn execute(
    shared: &Arc<Mutex<Sessions>>,
    request: PresetRequest,
) -> Result<SessionResponse, ApiError> {
    // Copy only the location while holding the editor lock. SQLite waits and
    // preset serialization must not block unrelated timeline edits.
    let database = shared
        .lock()
        .map_err(ApiError::io)?
        .review_database_path
        .clone();
    let path = database
        .parent()
        .ok_or_else(|| invalid("設定フォルダーがありません"))?
        .join("presets.sqlite");
    let mut library = PresetLibrary::open(&path).map_err(storage)?;
    let response = match request {
        PresetRequest::List { after_id, limit } => {
            if !(1..=20).contains(&limit) {
                return Err(invalid("一覧は1〜20件で指定してください"));
            }
            if let Some(after) = &after_id {
                id(after)?;
            }
            let mut records = library
                .list(after_id.as_deref().unwrap_or(""), limit + 1)
                .map_err(storage)?;
            let has_more = records.len() > limit as usize;
            records.truncate(limit as usize);
            let next_after_id = if has_more {
                records.last().map(|r| r.id.clone())
            } else {
                None
            };
            let items = records
                .into_iter()
                .map(|r| {
                    let r = record(r)?;
                    let preset = r.preset.ok_or_else(|| {
                        ApiError::new(
                            ErrorCode::IoError,
                            "削除済みプリセットが一覧に含まれています",
                        )
                    })?;
                    let kind = match preset.content {
                        PresetContent::Voice { .. } => PresetKind::Voice,
                        PresetContent::Character { .. } => PresetKind::Character,
                        PresetContent::Template { .. } => PresetKind::Template,
                    };
                    Ok(PresetSummary {
                        id: r.id,
                        revision: r.revision,
                        name: preset.name,
                        kind,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            SessionResponse::Presets {
                items,
                next_after_id,
            }
        }
        PresetRequest::Get { id: key } => {
            id(&key)?;
            let stored = library
                .get(&key)
                .map_err(storage)?
                .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "プリセットが見つかりません"))?;
            SessionResponse::Preset {
                record: record(stored)?,
            }
        }
        PresetRequest::Save {
            id: key,
            expected_revision,
            preset,
        } => {
            id(&key)?;
            preset.validate().map_err(|e| invalid(e.0))?;
            let bytes = serde_json::to_vec(&preset).map_err(invalid)?;
            if bytes.len() > 4 * 1024 * 1024 {
                return Err(invalid("プリセットは4MiB以内で保存してください"));
            }
            SessionResponse::Preset {
                record: record(
                    library
                        .write(&key, expected_revision, Some(&bytes))
                        .map_err(storage)?,
                )?,
            }
        }
        PresetRequest::Delete {
            id: key,
            expected_revision,
        } => {
            id(&key)?;
            SessionResponse::Preset {
                record: record(
                    library
                        .write(&key, expected_revision, None)
                        .map_err(storage)?,
                )?,
            }
        }
    };
    crate::session_errors::preflight(&response)?;
    Ok(response)
}

#[cfg(test)]
#[path = "preset_api_tests.rs"]
mod tests;
