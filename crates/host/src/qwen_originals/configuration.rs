//! Sungeki Maker-only profile input. Reading configuration never starts an engine.
use super::binding::Binding;
use crate::original_file;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path, sync::atomic::AtomicBool};
use sungeki_project::{Project, QwenIdentity, QwenSettings};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::qwen::{Catalog, QwenJobInput};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    pub id: String,
    pub name: String,
    pub binding: Binding,
    pub port: u16,
    pub identity: QwenIdentity,
    pub default_settings: QwenSettings,
    /// Expected metadata, compared to the owned engine after startup.
    pub catalog: [serde_json::Value; 3],
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
        return Err(invalid("Qwen設定IDが不正です"));
    }
    read_profiles(database, cancel)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "指定したQwen起動設定が見つかりません"))
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
        .join("qwen.json");
    if std::fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        let mut error = ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "Sungeki Maker用Qwen起動設定がありません",
        );
        error
            .details
            .insert("configuration_path".into(), path.display().to_string());
        error
            .suggested_actions
            .push("Sungeki Maker用のQwenモデル・起動環境を設定してください".into());
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
        return Err(invalid("Qwen設定が大きすぎます"));
    }
    let config: Configuration = serde_json::from_slice(bytes).map_err(invalid)?;
    if config.format_version != 1 || config.profiles.is_empty() || config.profiles.len() > 32 {
        return Err(invalid("Qwen設定の版または件数が不正です"));
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
            return Err(invalid("Qwen設定のID・名前・接続先が不正です"));
        }
        profile.identity.validate().map_err(invalid)?;
        profile.default_settings.validate().map_err(invalid)?;
        let selected = profile
            .catalog()?
            .select(&profile.default_settings)
            .map_err(invalid)?;
        if selected.backend != profile.identity.backend
            || selected.model_id != profile.identity.model_id
        {
            return Err(invalid("Qwen設定のカタログと原本識別情報が一致しません"));
        }
    }
    Ok(config.profiles)
}
impl Profile {
    pub(super) fn catalog(&self) -> Result<Catalog, ApiError> {
        Catalog::parse(
            self.catalog[0].clone(),
            self.catalog[1].clone(),
            self.catalog[2].clone(),
        )
        .map_err(invalid)
    }
    /// Validate originals off the IPC thread before scheduling. The worker repeats
    /// validation under its resource lease immediately before it starts the engine.
    pub fn prepare(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        cancel: &AtomicBool,
    ) -> Result<QwenJobInput, ApiError> {
        self.binding
            .checked_command(self.port, &self.identity, cancel)?;
        self.capture(revision, project, clip_id)
    }
    /// Capture the expected selection. The worker checks originals under its
    /// cancellable execution lease before starting any engine process.
    pub fn capture(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
    ) -> Result<QwenJobInput, ApiError> {
        let input = QwenJobInput::new(
            revision,
            project,
            clip_id,
            &format!("http://127.0.0.1:{}/", self.port),
            &self.identity,
            &self.catalog()?,
        )
        .map_err(invalid)?;
        input.for_profile(&self.id).map_err(invalid)
    }
}
