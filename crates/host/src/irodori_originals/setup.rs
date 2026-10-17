//! Produce a candidate from declared originals. Never install or start a model.
use super::{binding::Binding, configuration, registry};
use crate::original_file;
use serde::Deserialize;
use std::{path::Path, sync::atomic::AtomicBool};
use sungeki_project::{IrodoriIdentity, IrodoriSettings};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::irodori::ModelCapabilities;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Setup {
    format_version: u32,
    id: String,
    name: String,
    binding: Binding,
    port: u16,
    default_settings: IrodoriSettings,
    /// Declared capability only; compared to the owned runtime after startup.
    model: ModelCapabilities,
}
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
pub(crate) fn prepare(path: &Path, cancel: &AtomicBool) -> Result<serde_json::Value, ApiError> {
    let bytes = original_file::inspect(path, 32 * 1024 * 1024, true, cancel)?.bytes;
    let setup: Setup = serde_json::from_slice(&bytes).map_err(invalid)?;
    if setup.format_version != 1 {
        return Err(invalid("Irodori設定作成入力の版が不正です"));
    }
    let before = registry::read(
        &setup.binding.registry,
        &setup.binding.originals.voice,
        cancel,
    )?;
    let (_, measured) = setup
        .binding
        .measured_command(&setup.model.checkpoint, cancel)?;
    let after = registry::read(
        &setup.binding.registry,
        &setup.binding.originals.voice,
        cancel,
    )?;
    if before.sha256 != after.sha256 {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "Irodori設定作成中に登録音声が変更されました",
        ));
    }
    let identity = IrodoriIdentity {
        checkpoint: setup.model.checkpoint.clone(),
        model_manifest_sha256: measured.model_manifest_sha256,
        codec_manifest_sha256: measured.codec_manifest_sha256,
        engine_manifest_sha256: measured.engine_manifest_sha256,
        voice_manifest_sha256: measured.voice_manifest_sha256,
        sample_rate: setup.model.sample_rate,
        channels: setup.model.channels,
    };
    let profile = configuration::Profile {
        id: setup.id,
        name: setup.name,
        binding: setup.binding,
        port: setup.port,
        identity,
        default_settings: setup.default_settings,
        model: setup.model,
        voices: after.voices,
    };
    let candidate = serde_json::json!({"format_version":1,"profiles":[profile]});
    configuration::parse(&serde_json::to_vec(&candidate).map_err(invalid)?)?;
    Ok(candidate)
}
