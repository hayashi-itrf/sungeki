use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::{fs, path::PathBuf};

pub fn directory() -> std::io::Result<PathBuf> {
    let path = if let Some(path) = std::env::var_os("SUNGEKI_DATA_DIR") {
        PathBuf::from(path)
    } else {
        let home =
            std::env::var_os("HOME").ok_or_else(|| std::io::Error::other("HOMEが未設定です"))?;
        PathBuf::from(home).join(if cfg!(target_os = "macos") {
            "Library/Application Support/io.github.hayashi-itrf.sungeki"
        } else {
            ".local/share/sungeki"
        })
    };
    if !path.is_absolute() {
        return Err(std::io::Error::other("保存領域は絶対パスが必要です"));
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&path)?;
    let metadata = fs::symlink_metadata(&path)?;
    // SAFETY: geteuid has no arguments or failure mode.
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(std::io::Error::other(
            "保存領域の所有者・権限が不正です（0700が必要）",
        ));
    }
    Ok(path)
}
