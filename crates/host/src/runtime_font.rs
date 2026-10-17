//! Measure an explicitly selected font without replacing a supplied pin.
use super::{Settings, invalid};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
};
use sungeki_protocol::ApiError;
const MAX_FONT_BYTES: u64 = 128 * 1024 * 1024;
pub(super) fn complete(settings: &mut Settings) -> Result<(), ApiError> {
    let Some(path) = &settings.font else {
        return Ok(());
    };
    if settings.font_sha256.is_some() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(ApiError::io)?;
    let before = file.metadata().map_err(ApiError::io)?;
    if !before.is_file() || before.len() == 0 || before.len() > MAX_FONT_BYTES {
        return Err(invalid(
            "フォントは128MiB以内の空でない通常ファイルを選択してください",
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(ApiError::io)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > MAX_FONT_BYTES {
            return Err(invalid("フォントの読み込み中にサイズ上限を超えました"));
        }
        hasher.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(ApiError::io)?;
    let current = std::fs::symlink_metadata(path).map_err(ApiError::io)?;
    let stamp = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if count != before.len()
        || stamp(&before) != stamp(&after)
        || !current.is_file()
        || stamp(&after) != stamp(&current)
    {
        return Err(invalid(
            "確認中にフォントが変更されました。選び直してください",
        ));
    }
    settings.font_sha256 = Some(format!("{:x}", hasher.finalize()));
    Ok(())
}
