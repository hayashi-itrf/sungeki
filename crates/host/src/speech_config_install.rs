//! Install a validated Sungeki Maker configuration without replacing existing settings.
//! The temporary file is inspected in place, then published with no-clobber semantics.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};

pub(crate) fn install(
    directory: &Path,
    engine: &str,
    candidate: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf, ApiError> {
    let name = match engine {
        "sbv2" => "sbv2.json",
        "qwen" => "qwen.json",
        "irodori" => "irodori.json",
        _ => {
            return Err(ApiError::new(
                ErrorCode::ValidationFailed,
                "未対応の音声エンジンです",
            ));
        }
    };
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "Sungeki Maker設定フォルダーがありません",
        ));
    }
    let original = crate::original_file::inspect(candidate, 32 * 1024 * 1024, true, cancel)?;
    let temporary = directory.join(format!(".speech-config-{}.tmp", uuid::Uuid::new_v4()));
    let destination = directory.join(name);
    // Reject known conflicts before spending minutes inspecting model originals.
    // Publication still checks again: another registration can win meanwhile.
    match fs::symlink_metadata(&destination) {
        Ok(_) => check_existing(&destination, &original.bytes, cancel)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ApiError::io(error)),
    }
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(ApiError::io)?;
        file.write_all(&original.bytes).map_err(ApiError::io)?;
        file.sync_all().map_err(ApiError::io)?;
        drop(file);
        // Inspect the captured bytes, not a mutable source path. No model is started.
        match engine {
            "sbv2" => {
                crate::sbv2_originals::preflight::check(&temporary, cancel)?;
            }
            "qwen" => {
                crate::qwen_originals::preflight::check(&temporary, cancel)?;
            }
            "irodori" => {
                crate::irodori_originals::preflight::check(&temporary, cancel)?;
            }
            _ => unreachable!(),
        }
        let checked = crate::original_file::inspect(&temporary, 32 * 1024 * 1024, true, cancel)?;
        if checked.sha256 != original.sha256 {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "検査中に起動設定が変更されました",
            ));
        }
        publish(&temporary, &destination, &original.bytes, cancel)?;
        Ok(destination)
    })();
    let cleanup = fs::remove_file(&temporary);
    match (result, cleanup) {
        (Ok(path), Ok(())) => Ok(path),
        (Ok(_), Err(error)) => Err(ApiError::io(error)),
        (Err(error), _) => Err(error),
    }
}

fn publish(
    temporary: &Path,
    destination: &Path,
    bytes: &[u8],
    cancel: &AtomicBool,
) -> Result<(), ApiError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "起動設定の登録を取り消しました",
        ));
    }
    match fs::hard_link(temporary, destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            // Retry after response loss is safe only for the identical captured file.
            check_existing(destination, bytes, cancel)?;
        }
        Err(error) => return Err(ApiError::io(error)),
    }
    fs::File::open(destination.parent().unwrap())
        .and_then(|directory| directory.sync_all())
        .map_err(ApiError::io)
}

fn check_existing(destination: &Path, bytes: &[u8], cancel: &AtomicBool) -> Result<(), ApiError> {
    let existing = crate::original_file::inspect(destination, 32 * 1024 * 1024, true, cancel)?;
    if existing.bytes != bytes {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "既存の起動設定があります。登録では上書きしません",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "speech_config_install_tests.rs"]
mod tests;
