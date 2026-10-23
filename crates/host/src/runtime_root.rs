//! External runtime selection shared by output, review and permanent audio.
use std::{
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

pub(crate) fn configured(database: &Path) -> io::Result<PathBuf> {
    let saved = crate::runtime_settings::load(database).map_err(invalid)?;
    checked(&selected(
        std::env::var_os("SUNGEKI_RUNTIME_ROOT"),
        saved.runtime_root.as_deref(),
    )?)
}
pub(crate) fn selected(
    override_path: Option<std::ffi::OsString>,
    saved: Option<&Path>,
) -> io::Result<PathBuf> {
    crate::runtime_settings::select(override_path.map(PathBuf::from), saved.map(Path::to_owned)).0
        .ok_or_else(|| invalid("保存フォルダが未設定です。「設定」→「動画制作の実行環境」で外部ディスクの保存フォルダを選んで保存してください。"))
}
fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}
pub(crate) fn mount_for(root: &Path) -> io::Result<PathBuf> {
    let mut parts = root.components();
    if parts.next() != Some(Component::RootDir)
        || parts.next() != Some(Component::Normal("Volumes".as_ref()))
    {
        return Err(invalid("runtimeは外部ディスク内の絶対パスが必要です"));
    }
    let Some(Component::Normal(volume)) = parts.next() else {
        return Err(invalid("runtimeのディスク名が必要です"));
    };
    let remainder: Vec<_> = parts.collect();
    if remainder.is_empty() || remainder.iter().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(invalid(
            "runtimeはディスク内の専用フォルダーを指定してください",
        ));
    }
    Ok(Path::new("/Volumes").join(volume))
}
/// Read-only: setup must create the runtime explicitly. Never fall back to DB storage.
pub(crate) fn checked(root: &Path) -> io::Result<PathBuf> {
    let mount = mount_for(root)?;
    let mounted = fs::metadata(&mount).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("外部ディスクを確認できません: {}: {error}", mount.display()),
        )
    })?;
    if !mounted.is_dir() || mounted.dev() == fs::metadata("/Volumes")?.dev() {
        return Err(invalid("外部ディスクが接続されていません"));
    }
    let mount = fs::canonicalize(mount)?;
    let actual = fs::canonicalize(root).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "保存フォルダを確認できません。外部ディスク内の既存フォルダを選び直してください: {}: {error}",
                root.display()
            ),
        )
    })?;
    if actual == mount || !actual.starts_with(&mount) || !fs::metadata(&actual)?.is_dir() {
        return Err(invalid(
            "runtime実体は外部ディスク内の専用フォルダーに必要です",
        ));
    }
    Ok(actual)
}

#[cfg(test)]
#[path = "runtime_root_tests.rs"]
mod tests;
