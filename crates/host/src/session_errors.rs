use sungeki_editor::{HistoryError, OperationError};
use sungeki_protocol::*;
use sungeki_storage::StorageError;

pub(crate) fn storage_error(error: StorageError) -> ApiError {
    let code = if matches!(error, StorageError::Conflict) {
        ErrorCode::Conflict
    } else {
        ErrorCode::IoError
    };
    ApiError::new(code, format!("復旧DB: {error:?}"))
}
pub(crate) fn edit_error(error: OperationError) -> ApiError {
    let mut details = std::collections::BTreeMap::new();
    if let OperationError::History(HistoryError::Validation(validation)) = &error
        && let Some(sungeki_project::ValidationDiagnostic::TrackOverlap {
            track_id,
            track_name,
            first_clip_id,
            second_clip_id,
        }) = &validation.1
    {
        details.extend([
            ("reason".into(), "track_overlap".into()),
            ("track_id".into(), track_id.clone()),
            ("track_name".into(), track_name.clone()),
            ("first_clip_id".into(), first_clip_id.clone()),
            ("second_clip_id".into(), second_clip_id.clone()),
        ]);
    }
    let code = match &error {
        OperationError::History(HistoryError::Conflict)
        | OperationError::WrongUndoTarget
        | OperationError::ReusedId => ErrorCode::Conflict,
        OperationError::InvalidSnapshot(_) => ErrorCode::IoError,
        _ => ErrorCode::ValidationFailed,
    };
    let message = match error {
        OperationError::History(HistoryError::Conflict) => {
            "別の編集で作品が更新されたため、この変更は適用されませんでした。".into()
        }
        OperationError::WrongUndoTarget => {
            "取り消し・やり直しの対象が変わりました。操作履歴を確認してください。".into()
        }
        OperationError::ReusedId => {
            "同じ操作IDが異なる編集内容に使われています。新しい操作IDを指定してください。".into()
        }
        OperationError::InvalidId => "操作IDには標準形式のUUIDを指定してください。".into(),
        OperationError::History(HistoryError::Empty) => {
            "取り消し・やり直しできる操作がありません。".into()
        }
        OperationError::History(HistoryError::RevisionLimit) => {
            "この作品の編集回数の上限に達しました。".into()
        }
        OperationError::History(HistoryError::Validation(error)) => error.0,
        OperationError::InvalidSnapshot(message) => format!("編集状態を復元できません: {message}"),
    };
    let mut result = ApiError::new(code, message);
    result.details = details;
    result
}
pub(crate) fn preflight(response: &SessionResponse) -> Result<(), ApiError> {
    if serde_json::to_vec(response).map_err(ApiError::io)?.len() + 512 > framing::MAX_FRAME_BYTES {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "現在の文書通信上限を超えます",
        ));
    }
    Ok(())
}
