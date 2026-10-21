//! Sungeki Maker-only profile input. Reading configuration never starts an engine.
use super::binding::Binding;
use crate::original_file;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path, sync::atomic::AtomicBool};
use sungeki_project::{IrodoriIdentity, IrodoriSettings, Project};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::irodori::{Catalog, IrodoriJobInput, ModelCapabilities, RegisteredVoice};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    pub id: String,
    pub name: String,
    pub binding: Binding,
    pub port: u16,
    pub identity: IrodoriIdentity,
    pub default_settings: IrodoriSettings,
    /// Expected metadata, compared to the owned engine after startup.
    pub model: ModelCapabilities,
    pub voices: Vec<RegisteredVoice>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    format_version: u32,
    profiles: Vec<Profile>,
}
fn invalid(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::ValidationFailed, message.to_string())
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
pub(crate) fn load(database: &Path, id: &str, cancel: &AtomicBool) -> Result<Profile, ApiError> {
    if !valid_id(id) {
        return Err(invalid("Irodori設定IDが不正です"));
    }
    read_profiles(database, cancel)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                "指定したIrodori起動設定が見つかりません",
            )
        })
}
pub(crate) fn read_profiles(
    database: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<Profile>, ApiError> {
    if !database.is_absolute() {
        return Err(invalid("Sungeki Maker保存領域には絶対パスが必要です"));
    }
    let path = database
        .parent()
        .ok_or_else(|| invalid("Sungeki Maker保存領域がありません"))?
        .join("irodori.json");
    if std::fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        let mut error = ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "Sungeki Maker用Irodori起動設定がありません",
        );
        error
            .details
            .insert("configuration_path".into(), path.display().to_string());
        error
            .suggested_actions
            .push("Sungeki Maker用のIrodoriモデル・起動環境を設定してください".into());
        return Err(error);
    }
    read_candidate(&path, cancel)
}
/// Blocking setup/registration worker. No fallback to the previous application's settings.
pub(crate) fn read_candidate(path: &Path, cancel: &AtomicBool) -> Result<Vec<Profile>, ApiError> {
    let bytes = original_file::inspect(path, 32 * 1024 * 1024, true, cancel)?.bytes;
    parse(&bytes)
}
pub(super) fn parse(bytes: &[u8]) -> Result<Vec<Profile>, ApiError> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(invalid("Irodori設定が大きすぎます"));
    }
    let config: Configuration = serde_json::from_slice(bytes).map_err(invalid)?;
    if config.format_version != 1 || config.profiles.is_empty() || config.profiles.len() > 32 {
        return Err(invalid("Irodori設定の版または件数が不正です"));
    }
    let mut ids = BTreeSet::new();
    for profile in &config.profiles {
        if !valid_id(&profile.id)
            || !ids.insert(&profile.id)
            || profile.name.trim().is_empty()
            || profile.name.len() > 256
            || profile.name.chars().any(char::is_control)
            || profile.port < 1024
        {
            return Err(invalid("Irodori設定のID・名前・接続先が不正です"));
        }
        profile.identity.validate().map_err(invalid)?;
        profile.default_settings.validate().map_err(invalid)?;
        profile
            .catalog()?
            .select(&profile.default_settings, &profile.identity)
            .map_err(invalid)?;
        if profile.identity.channels != 1 {
            return Err(invalid("現在のIrodori workerはmonoのみ対応します"));
        }
    }
    Ok(config.profiles)
}
impl Profile {
    pub(super) fn catalog(&self) -> Result<Catalog, ApiError> {
        Catalog::new(self.model.clone(), self.voices.clone()).map_err(invalid)
    }
    pub fn checked_catalog(&self, cancel: &AtomicBool) -> Result<Catalog, ApiError> {
        let before = super::registry::read(
            &self.binding.registry,
            &self.binding.originals.voice,
            cancel,
        )?;
        self.binding.checked_command(&self.identity, cancel)?;
        let after = super::registry::read(
            &self.binding.registry,
            &self.binding.originals.voice,
            cancel,
        )?;
        if before.sha256 != after.sha256 {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "Irodori登録音声が検査中に変わりました",
            ));
        }
        self.match_registry(after.voices)
    }
    fn match_registry(&self, voices: Vec<RegisteredVoice>) -> Result<Catalog, ApiError> {
        let expected = self.catalog()?;
        let actual = Catalog::new(self.model.clone(), voices).map_err(invalid)?;
        if !expected.voices().eq(actual.voices()) {
            return Err(ApiError::new(
                ErrorCode::ExternalModification,
                "Irodori設定と登録音声の内容が一致しません",
            ));
        }
        Ok(actual)
    }
    /// Validate originals off the IPC thread before scheduling. The worker repeats
    /// validation under its resource lease immediately before it starts the engine.
    pub fn prepare(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        cancel: &AtomicBool,
    ) -> Result<IrodoriJobInput, ApiError> {
        let catalog = self.checked_catalog(cancel)?;
        let input = IrodoriJobInput::new(revision, project, clip_id, &self.identity, &catalog)
            .map_err(invalid)?;
        input.for_profile(&self.id).map_err(invalid)
    }
    /// Only bounded registry metadata is read before scheduling. Model/runtime
    /// and reference hashes remain mandatory in the cancellable worker.
    pub fn capture(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        cancel: &AtomicBool,
    ) -> Result<IrodoriJobInput, ApiError> {
        let registry = super::registry::read(
            &self.binding.registry,
            &self.binding.originals.voice,
            cancel,
        )?;
        let catalog = self.match_registry(registry.voices)?;
        IrodoriJobInput::new(revision, project, clip_id, &self.identity, &catalog)
            .map_err(invalid)?
            .for_profile(&self.id)
            .map_err(invalid)
    }
}

#[cfg(test)]
mod tests;
