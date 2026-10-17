//! Structured failures shared by live responses and durable job records.
use super::Error;
use sungeki_assets::{CollectionError, CollectionPlan, CollectionSourceStatus};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_storage::PublishError;
pub(super) fn api(error: Error) -> ApiError {
    match error {
        Error::Api(error) => error,
        Error::Io(error) | Error::Publish(PublishError::Io(error)) => io_error(error),
        Error::Document(sungeki_storage::DocumentError::Io(error))
            if error.kind() == std::io::ErrorKind::StorageFull =>
        {
            io_error(error)
        }
        Error::Document(error) => crate::session_save::document_error(error),
        Error::Plan(CollectionError::Cancelled) | Error::Publish(PublishError::Cancelled) => {
            ApiError::new(
                ErrorCode::Cancelled,
                "素材収集は保存先の確定前に取り消されました",
            )
        }
        Error::Plan(CollectionError::Invalid(message)) => {
            ApiError::new(ErrorCode::ValidationFailed, message)
        }
        Error::Publish(PublishError::OutputExists) => ApiError::new(
            ErrorCode::OutputExists,
            "保存先が既に存在します。別の新しいフォルダを指定してください",
        ),
        Error::Publish(PublishError::Changed) => ApiError::new(
            ErrorCode::ExternalModification,
            "収集中に素材または保存先が変更されました。素材を確認して再実行してください",
        ),
        Error::Publish(
            PublishError::InvalidDestination
            | PublishError::InvalidFingerprint
            | PublishError::InvalidRange,
        ) => ApiError::new(
            ErrorCode::ValidationFailed,
            "保存先または素材の識別情報が不正です",
        ),
        Error::Publish(PublishError::DurabilityUncertain { path, source }) => {
            let mut error = ApiError::io(format!(
                "収集フォルダは存在しますが保存の同期を確認できません: {source}"
            ));
            error
                .details
                .insert("path".into(), path.to_string_lossy().into());
            error
                .details
                .insert("publication".into(), "durability_uncertain".into());
            error
        }
        Error::SourcesNotReady => {
            ApiError::new(ErrorCode::AssetMissing, "収集に必要な素材が揃っていません")
        }
        Error::Limit(message) => ApiError::new(ErrorCode::ValidationFailed, message),
        Error::Invalid(message) => ApiError::new(ErrorCode::IoError, message),
    }
}
fn io_error(error: std::io::Error) -> ApiError {
    if error.kind() != std::io::ErrorKind::StorageFull {
        return ApiError::io(error);
    }
    let mut result = ApiError::new(
        ErrorCode::IoError,
        "保存先の空き容量が不足しています。空き容量を増やすか別の保存先を選び、「コピーせず事前確認」からやり直してください。",
    );
    result
        .details
        .insert("reason".into(), "insufficient_space".into());
    result.details.insert("cause".into(), error.to_string());
    if let Some(code) = error.raw_os_error() {
        result.details.insert("os_error".into(), code.to_string());
    }
    result
}

#[cfg(test)]
#[path = "collection_error_tests.rs"]
mod tests;
pub(super) fn sources(plan: &CollectionPlan) -> ApiError {
    let mut missing = 0;
    let mut changed = 0;
    let mut unreadable = 0;
    let mut uninspected = 0;
    let mut unresolved = 0;
    let mut conflicts = 0;
    for file in &plan.files {
        if file.conflicting_metadata {
            conflicts += 1;
        }
        if file
            .source_indices
            .iter()
            .any(|i| plan.sources[*i].status == CollectionSourceStatus::Verified)
        {
            continue;
        }
        for i in &file.source_indices {
            match &plan.sources[*i].status {
                CollectionSourceStatus::Missing => missing += 1,
                CollectionSourceStatus::Changed => changed += 1,
                CollectionSourceStatus::Unreadable(_) => unreadable += 1,
                CollectionSourceStatus::NotInspected => uninspected += 1,
                CollectionSourceStatus::Unresolved(_) => unresolved += 1,
                CollectionSourceStatus::Verified => {}
            }
        }
    }
    let (code, message) = if conflicts > 0 {
        (
            ErrorCode::ValidationFailed,
            "同じ素材の登録情報に矛盾があります",
        )
    } else if changed > 0 {
        (
            ErrorCode::ExternalModification,
            "登録後に素材の内容が変更されています",
        )
    } else if missing > 0 {
        (ErrorCode::AssetMissing, "収集に必要な素材が見つかりません")
    } else if unreadable > 0 {
        (ErrorCode::IoError, "収集に必要な素材を読み込めません")
    } else if unresolved > 0 {
        (
            ErrorCode::ValidationFailed,
            "収集素材の参照先を解決できません",
        )
    } else {
        (
            ErrorCode::CapabilityUnavailable,
            "検査上限を超えた素材があり、収集を確定できません",
        )
    };
    let mut result = ApiError::new(code, message);
    for (key, value) in [
        ("missing", missing),
        ("changed", changed),
        ("unreadable", unreadable),
        ("not_inspected", uninspected),
        ("unresolved", unresolved),
        ("conflicting_metadata", conflicts),
    ] {
        result.details.insert(key.into(), value.to_string());
    }
    result
}
