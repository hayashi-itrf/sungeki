#[path = "original_python.rs"]
pub(crate) mod python;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, Metadata, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};
pub(crate) struct Fingerprint {
    pub sha256: String,
    pub len: u64,
    pub bytes: Vec<u8>,
}
pub(crate) fn inspect(
    path: &Path,
    limit: u64,
    capture: bool,
    cancel: &AtomicBool,
) -> Result<Fingerprint, ApiError> {
    read(path, limit, capture, false, cancel)
}
/// Empty Python modules still affect the manifest and must be hashed.
pub(crate) fn inspect_source(
    path: &Path,
    limit: u64,
    cancel: &AtomicBool,
) -> Result<Fingerprint, ApiError> {
    read(path, limit, false, true, cancel)
}
fn read(
    path: &Path,
    limit: u64,
    capture: bool,
    allow_empty: bool,
    cancel: &AtomicBool,
) -> Result<Fingerprint, ApiError> {
    cancelled(cancel)?;
    if !path.is_absolute() || capture && limit > 32 * 1024 * 1024 {
        return Err(invalid(
            "音声エンジン原本の絶対パスまたは読込上限が不正です",
        ));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(ApiError::io)?;
    let before = file.metadata().map_err(ApiError::io)?;
    if !before.is_file() || !allow_empty && before.len() == 0 || before.len() > limit {
        return Err(changed("音声エンジン原本の種類または容量が不正です"));
    }
    let mut hasher = Sha256::new();
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        cancelled(cancel)?;
        let n = file.read(&mut buffer).map_err(ApiError::io)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > before.len() {
            return Err(changed("読込中に音声エンジン原本の容量が変わりました"));
        }
        hasher.update(&buffer[..n]);
        if capture {
            bytes.extend_from_slice(&buffer[..n]);
        }
    }
    let after = file.metadata().map_err(ApiError::io)?;
    let current = fs::symlink_metadata(path).map_err(ApiError::io)?;
    if !current.is_file()
        || total != before.len()
        || stamp(&before) != stamp(&after)
        || stamp(&after) != stamp(&current)
    {
        return Err(changed("読込中に音声エンジン原本が変更・置換されました"));
    }
    cancelled(cancel)?;
    Ok(Fingerprint {
        sha256: format!("{:x}", hasher.finalize()),
        len: total,
        bytes,
    })
}
fn cancelled(cancel: &AtomicBool) -> Result<(), ApiError> {
    if cancel.load(Ordering::Acquire) {
        Err(ApiError::new(
            ErrorCode::Cancelled,
            "音声エンジン原本の照合を取り消しました",
        ))
    } else {
        Ok(())
    }
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}

fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
fn changed(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ExternalModification, message)
}
