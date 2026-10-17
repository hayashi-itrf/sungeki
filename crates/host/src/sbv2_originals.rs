//! Inspect explicitly configured originals. Never follow paths returned by HTTP.
//! These measurements do not prove which files a running process has loaded.
mod binding;
pub(crate) mod configuration;
pub(crate) mod discovery;
mod engine;
use crate::original_file as file;
pub(crate) mod preflight;
use crate::original_file::python;
pub(crate) use binding::Binding;
pub(crate) use engine::EngineFiles;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};
use sungeki_project::Sbv2Identity;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::sbv2::ModelConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Paths {
    pub model: PathBuf,
    pub config: PathBuf,
    pub style_vectors: PathBuf,
    /// Complete loader dependency list, keyed by stable logical names, not absolute paths.
    /// The runtime owner must establish completeness; hashing a supplied list cannot.
    pub text_dependencies: BTreeMap<String, PathBuf>,
}
pub(crate) struct Measured {
    pub model_sha256: String,
    pub config_sha256: String,
    pub style_vectors_sha256: String,
    pub text_encoder_fingerprint: String,
    pub config_text: String,
}
impl Measured {
    /// Checks file-derived identity fields only. Engine revision/process ownership
    /// must be checked separately; a Git revision string alone is not evidence.
    pub fn verify_files(&self, expected: &Sbv2Identity) -> Result<(), ApiError> {
        expected.validate().map_err(invalid)?;
        if self.model_sha256 != expected.model_sha256
            || self.config_sha256 != expected.config_sha256
            || self.style_vectors_sha256 != expected.style_vectors_sha256
            || self.text_encoder_fingerprint != expected.text_encoder_fingerprint
        {
            return Err(changed("SBV2の原本が保存された識別情報と一致しません"));
        }
        Ok(())
    }
}
/// Blocking worker only; memory use stays bounded even for multi-GB weights.
pub(crate) fn inspect(paths: &Paths, cancel: &AtomicBool) -> Result<Measured, ApiError> {
    if paths.text_dependencies.is_empty()
        || paths.text_dependencies.len() > 128
        || paths.text_dependencies.keys().any(|key| {
            key.trim().is_empty() || key.len() > 1024 || key.chars().any(char::is_control)
        })
    {
        return Err(invalid("SBV2の言語処理依存一覧が不正です"));
    }
    let config = file::inspect(&paths.config, 4 * 1024 * 1024, true, cancel)?;
    ModelConfig::parse(&config.bytes).map_err(invalid)?;
    let config_text = String::from_utf8(config.bytes).map_err(invalid)?;
    let model = file::inspect(&paths.model, 8 * 1024 * 1024 * 1024, false, cancel)?;
    let styles = file::inspect(&paths.style_vectors, 256 * 1024 * 1024, false, cancel)?;
    let mut dependencies = Vec::new();
    let mut remaining = 32u64 * 1024 * 1024 * 1024;
    for (name, path) in &paths.text_dependencies {
        let value = file::inspect(path, remaining.min(8 * 1024 * 1024 * 1024), false, cancel)?;
        remaining -= value.len;
        dependencies.push((name, value.sha256));
    }
    let manifest =
        serde_json::to_vec(&("vvm-sbv2-text-dependencies-v1", dependencies)).map_err(invalid)?;
    Ok(Measured {
        model_sha256: model.sha256,
        config_sha256: config.sha256,
        style_vectors_sha256: styles.sha256,
        config_text,
        text_encoder_fingerprint: format!("{:x}", Sha256::digest(manifest)),
    })
}
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
fn changed(message: &str) -> ApiError {
    ApiError::new(ErrorCode::ExternalModification, message)
}

#[cfg(test)]
mod tests;
