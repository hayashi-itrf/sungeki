use crate::review_record::invalid;
use std::{
    fs,
    path::{Path, PathBuf},
};
use sungeki_protocol::{ApiError, ErrorCode};

/// This rejection occurs before create_job; it is not an uncertain I/O commit.
pub(crate) fn unavailable(cause: &ApiError) -> ApiError {
    let mut error = ApiError::new(
        ErrorCode::CapabilityUnavailable,
        "画像検査の作業領域を利用できません。SSDの接続と保存先を確認して、もう一度開始してください。",
    );
    error.retryable = true;
    error.details.insert("reason".into(), cause.message.clone());
    error.details.insert("job_created".into(), "false".into());
    error.suggested_actions = vec![
        "「設定」→「動画制作の実行環境」で外部ディスクの保存フォルダを選んで保存する".into(),
        "SSDの接続と作業領域の設定を確認する".into(),
        "修復後に再度確認を開始する（起動時の環境変数を変更した場合のみホストの再起動が必要）"
            .into(),
    ];
    error
}

pub(crate) fn root(database: &Path) -> Result<PathBuf, ApiError> {
    let parent = database
        .parent()
        .ok_or_else(|| invalid("DB親フォルダーがありません"))?;
    let parent = fs::canonicalize(parent).map_err(invalid)?;
    let mut name = database
        .file_name()
        .ok_or_else(|| invalid("DB名がありません"))?
        .to_os_string();
    name.push(".review-work");
    let root = if cfg!(test) {
        parent.join(name)
    } else if let Some(path) = std::env::var_os("SUNGEKI_REVIEW_WORK_ROOT") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(invalid("検査作業領域は絶対パスが必要です"));
        }
        path
    } else {
        configured_root(&parent.join(database.file_name().unwrap()))?
    };
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(&root).map_err(invalid)?.is_dir() {
                return Err(invalid("検査用フォルダーが通常のフォルダーではありません"));
            }
        }
        Err(error) => return Err(invalid(error)),
    }
    Ok(root)
}
fn configured_root(database: &Path) -> Result<PathBuf, ApiError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;
    let runtime = crate::runtime_root::configured(&database).map_err(invalid)?;
    // Distinct recovery databases must not recover each other's job directories.
    let profile = format!("{:x}", Sha256::digest(database.as_os_str().as_bytes()));
    Ok(runtime.join(format!("review-work-{profile}")))
}
pub(crate) fn path(root: &Path, id: &str) -> Result<PathBuf, ApiError> {
    if uuid::Uuid::parse_str(id)
        .ok()
        .is_none_or(|id_value| id_value.to_string() != id)
    {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "作業領域には標準形式のジョブIDが必要です",
        ));
    }
    Ok(root.join(id))
}
/// Called only on host startup while the profile's exclusive host lock is held.
/// Remove only work directories belonging to known persisted job IDs.
pub(crate) fn recover(root: &Path, id: &str) -> Result<(), ApiError> {
    let path = path(root, id)?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(invalid(error)),
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(invalid),
        Ok(_) => Err(invalid("検査作業領域が別のファイルへ置換されています")),
    }
}
