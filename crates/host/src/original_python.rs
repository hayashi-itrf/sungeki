//! Keep the venv invocation path separate from the binary whose bytes are inspected.
use super::{changed, invalid};
use crate::original_file as file;
use std::{os::unix::fs::PermissionsExt, path::Path, sync::atomic::AtomicBool};
use sungeki_protocol::ApiError;
pub(crate) fn inspect(
    executable: &Path,
    target: &Path,
    cancel: &AtomicBool,
) -> Result<(String, String), ApiError> {
    if !executable.is_absolute()
        || !target.is_absolute()
        || [executable, target]
            .iter()
            .any(|p| p.components().any(|c| c == std::path::Component::ParentDir))
        || executable.file_name().is_none_or(|name| name != "python")
    {
        return Err(invalid(
            "音声エンジンには仮想環境のbin/pythonと実体の絶対パスが必要です",
        ));
    }
    let bin = executable
        .parent()
        .ok_or_else(|| invalid("Pythonの親がありません"))?;
    if bin.file_name().is_none_or(|name| name != "bin") {
        return Err(invalid("Pythonの起動パスは仮想環境のbin/pythonが必要です"));
    }
    let root = bin
        .parent()
        .ok_or_else(|| invalid("仮想環境のルートがありません"))?;
    let before = std::fs::canonicalize(executable).map_err(ApiError::io)?;
    if before != target {
        return Err(changed("Pythonのリンク先が設定された実体と一致しません"));
    }
    let binary = file::inspect(target, 512 * 1024 * 1024, false, cancel)?;
    if std::fs::symlink_metadata(target)
        .map_err(ApiError::io)?
        .permissions()
        .mode()
        & 0o111
        == 0
    {
        return Err(invalid("音声エンジンのPythonに実行権限がありません"));
    }
    let config = file::inspect(&root.join("pyvenv.cfg"), 65536, true, cancel)?;
    let text = std::str::from_utf8(&config.bytes).map_err(invalid)?;
    let mut system_site =
        text.lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| {
                key.trim()
                    .eq_ignore_ascii_case("include-system-site-packages")
            });
    if !system_site
        .next()
        .is_some_and(|(_, value)| value.trim().eq_ignore_ascii_case("false"))
        || system_site.next().is_some()
    {
        return Err(invalid(
            "音声エンジン仮想環境のシステムパッケージ共有は無効にしてください",
        ));
    }
    if std::fs::canonicalize(executable).map_err(ApiError::io)? != before {
        return Err(changed("検査中にPythonのリンク先が変更されました"));
    }
    Ok((binary.sha256, config.sha256))
}

#[cfg(test)]
#[path = "sbv2_originals/python/tests.rs"]
mod tests;
