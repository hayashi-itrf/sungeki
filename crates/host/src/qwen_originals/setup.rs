//! Derive a candidate identity from explicit originals, without installing it.
use super::{binding::Binding, configuration, model_files};
use crate::original_file;
use serde::Deserialize;
use std::{path::Path, sync::atomic::AtomicBool};
use sungeki_project::{QwenIdentity, QwenSettings};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::qwen::Catalog;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    format_version: u32,
    id: String,
    name: String,
    binding: Binding,
    port: u16,
    default_settings: QwenSettings,
    catalog: [serde_json::Value; 3],
    /// Declared model output format; never inferred from a generated WAV.
    sample_rate: u32,
    channels: u16,
}
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
pub(crate) fn prepare(path: &Path, cancel: &AtomicBool) -> Result<serde_json::Value, ApiError> {
    let bytes = original_file::inspect(path, 32 * 1024 * 1024, true, cancel)?.bytes;
    let mut setup: Setup = serde_json::from_slice(&bytes).map_err(invalid)?;
    if setup.format_version != 1 {
        return Err(invalid("Qwen設定作成入力の版が不正です"));
    }
    let catalog = Catalog::parse(
        setup.catalog[0].clone(),
        setup.catalog[1].clone(),
        setup.catalog[2].clone(),
    )
    .map_err(invalid)?;
    let selected = catalog.select(&setup.default_settings).map_err(invalid)?;
    // The setup's model list must be empty: never silently replace a supplied list
    // that a human may have intended to pin. Model inventory is derived locally.
    if !setup.binding.originals.model.is_empty() {
        return Err(invalid(
            "モデル一覧は自動取得します。setupのmodel一覧は空にしてください",
        ));
    }
    setup.binding.originals.model = model_files::discover(&setup.binding.model_root, cancel)?;
    let (_, measured) = setup.binding.measured_command(
        setup.port,
        &selected.backend,
        &selected.model_id,
        cancel,
    )?;
    let identity = QwenIdentity {
        backend: selected.backend,
        model_id: selected.model_id,
        model_manifest_sha256: measured.model_manifest_sha256,
        engine_manifest_sha256: measured.engine_manifest_sha256,
        voice_manifest_sha256: measured.voice_manifest_sha256,
        sample_rate: setup.sample_rate,
        channels: setup.channels,
    };
    let profile = configuration::Profile {
        id: setup.id,
        name: setup.name,
        binding: setup.binding,
        port: setup.port,
        default_settings: setup.default_settings,
        catalog: catalog.snapshot().map_err(invalid)?,
        identity,
    };
    let candidate = serde_json::json!({"format_version":1,"profiles":[profile]});
    let bytes = serde_json::to_vec(&candidate).map_err(invalid)?;
    // Original/layout inspection above already produced these hashes. Validate
    // the resulting configuration without rereading all model/runtime bytes.
    // Preflight and every managed launch still inspect originals independently.
    configuration::parse(&bytes)?;
    Ok(candidate)
}
