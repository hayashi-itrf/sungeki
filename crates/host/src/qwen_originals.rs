//! Explicit file manifests only. Never follow paths supplied by a server.
//! The managed launcher must establish completeness and loaded-process identity.
pub(crate) mod api;
pub(crate) mod binding;
pub(crate) mod configuration;
pub(crate) mod exchange;
pub(crate) mod job;
mod model_files;
pub(crate) mod preflight;
pub(crate) mod profiles;
pub(crate) mod setup;
pub(crate) mod startup;
use crate::original_file;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::qwen::RuntimeIdentity;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Paths {
    /// Includes weight shards, configuration and tokenizer assets used by loader.
    pub model: BTreeMap<String, PathBuf>,
    /// Includes sources, executable and pinned runtime dependencies.
    pub engine: BTreeMap<String, PathBuf>,
    /// Includes selected built-in mapping or clone audio, transcript and config.
    pub voice: BTreeMap<String, PathBuf>,
}
pub(crate) struct Measured {
    pub model_manifest_sha256: String,
    pub engine_manifest_sha256: String,
    pub voice_manifest_sha256: String,
}
impl Measured {
    /// This does not attest backend, loaded model ID or output format.
    pub fn verify_files(&self, expected: &RuntimeIdentity) -> Result<(), ApiError> {
        expected.validate().map_err(|_| invalid())?;
        if self.model_manifest_sha256 != expected.model_manifest_sha256
            || self.engine_manifest_sha256 != expected.engine_manifest_sha256
            || self.voice_manifest_sha256 != expected.voice_manifest_sha256
        {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "Qwenの原本が保存された識別情報と一致しません",
            ));
        }
        Ok(())
    }
}
/// Blocking worker; reads large weights in fixed buffers and honors cancellation.
pub(crate) fn inspect(paths: &Paths, cancel: &AtomicBool) -> Result<Measured, ApiError> {
    Ok(Measured {
        model_manifest_sha256: manifest(
            "model",
            &paths.model,
            32 * 1024 * 1024 * 1024,
            false,
            cancel,
        )?,
        engine_manifest_sha256: manifest(
            "engine",
            &paths.engine,
            8 * 1024 * 1024 * 1024,
            true,
            cancel,
        )?,
        voice_manifest_sha256: manifest("voice", &paths.voice, 1024 * 1024 * 1024, false, cancel)?,
    })
}
fn manifest(
    role: &str,
    files: &BTreeMap<String, PathBuf>,
    mut remaining: u64,
    allow_empty: bool,
    cancel: &AtomicBool,
) -> Result<String, ApiError> {
    if cancel.load(Ordering::Acquire) {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "Qwen原本の照合を取り消しました",
        ));
    }
    if files.is_empty()
        || files.len() > 65536
        || files.keys().any(|name| {
            name.trim().is_empty() || name.len() > 1024 || name.chars().any(char::is_control)
        })
    {
        return Err(invalid());
    }
    let mut measured = Vec::with_capacity(files.len());
    for (name, path) in files {
        let limit = remaining.min(8 * 1024 * 1024 * 1024);
        let value = if allow_empty {
            original_file::inspect_source(path, limit, cancel)?
        } else {
            original_file::inspect(path, limit, false, cancel)?
        };
        remaining -= value.len;
        measured.push((name, value.len, value.sha256));
    }
    let bytes = serde_json::to_vec(&("vvm-qwen-original-manifest-v1", role, measured))
        .map_err(|_| invalid())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn invalid() -> ApiError {
    ApiError::new(
        ErrorCode::ValidationFailed,
        "Qwen原本のファイル一覧または識別情報が不正です",
    )
}
#[cfg(test)]
mod tests;
