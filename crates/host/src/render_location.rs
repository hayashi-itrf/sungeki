use crate::render_record::invalid;
use std::{
    fs,
    path::{Path, PathBuf},
};
use sungeki_protocol::{ApiError, ErrorCode};

/// Only for failures before the output job is persisted; retry is unambiguous.
pub(crate) fn unavailable(cause: ApiError) -> ApiError {
    let mut error = ApiError::new(
        ErrorCode::CapabilityUnavailable,
        "動画出力の作業領域を利用できません。SSDの接続、保存先フォルダーと書き込み権限を確認して、もう一度開始してください。",
    );
    error.retryable = true;
    error.details.insert("reason".into(), cause.message);
    error.details.insert("job_created".into(), "false".into());
    error.suggested_actions = vec![
        "「設定」→「動画制作の実行環境」で外部ディスクの保存フォルダを選んで保存する".into(),
        "SSDの接続、作業領域の保存先と書き込み権限を確認する".into(),
        "修復後に動画出力を開始し直す（起動時の保存先設定を変更した場合はホストの再起動が必要）"
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
    name.push(".render-work");
    let root = if cfg!(test) {
        parent.join(name)
    } else if let Some(path) = std::env::var_os("SUNGEKI_RENDER_WORK_ROOT") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(invalid("描画作業領域は絶対パスが必要です"));
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
                return Err(invalid("描画用フォルダーが通常のフォルダーではありません"));
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
    Ok(runtime.join(format!("render-work-{profile}")))
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
