//! Sungeki Maker-only local configuration. No old-app fallback and no engine execution.
use super::{Binding, file, invalid};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path, sync::atomic::AtomicBool};
use sungeki_project::{ClipContent, Project, Voice};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::sbv2::{Catalog, Sbv2JobInput};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    pub id: String,
    pub name: String,
    pub binding: Binding,
    pub cpu_only: bool,
    /// Expected loader metadata captured during installation/model selection.
    /// It is compared with the owned engine after boot, never trusted as a hash.
    pub catalog: serde_json::Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    format_version: u32,
    profiles: Vec<Profile>,
}
fn profile_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
/// Blocking worker only. The host supplies its DB path; clients select an ID,
/// not an arbitrary file. Keep model/runtime paths on their configured volume.
pub(crate) fn load(database: &Path, id: &str, cancel: &AtomicBool) -> Result<Profile, ApiError> {
    if !profile_id(id) {
        return Err(invalid("SBV2設定IDまたはSungeki Maker保存領域が不正です"));
    }
    read_profiles(database, cancel)?
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "指定したSBV2起動設定が見つかりません"))
}
pub(crate) fn summaries(
    database: &Path,
    cancel: &AtomicBool,
) -> Result<Vec<sungeki_protocol::Sbv2ProfileSummary>, ApiError> {
    Ok(read_profiles(database, cancel)?
        .into_iter()
        .map(|profile| sungeki_protocol::Sbv2ProfileSummary {
            id: profile.id,
            name: profile.name,
            cpu_only: profile.cpu_only,
        })
        .collect())
}
fn read_profiles(database: &Path, cancel: &AtomicBool) -> Result<Vec<Profile>, ApiError> {
    if !database.is_absolute() {
        return Err(invalid("Sungeki Maker保存領域には絶対パスが必要です"));
    }
    let parent = database
        .parent()
        .ok_or_else(|| invalid("Sungeki Maker保存領域がありません"))?;
    let path = parent.join("sbv2.json");
    if std::fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        let mut error = ApiError::new(
            ErrorCode::CapabilityUnavailable,
            "Sungeki Maker用SBV2起動設定がありません",
        );
        error
            .details
            .insert("configuration_path".into(), path.display().to_string());
        error
            .suggested_actions
            .push("Sungeki Maker用のSBV2モデル・起動環境を設定してください".into());
        return Err(error);
    }
    read_candidate(&path, cancel)
}
/// Offline setup validation only. Public session requests still select a saved ID.
pub(crate) fn read_candidate(path: &Path, cancel: &AtomicBool) -> Result<Vec<Profile>, ApiError> {
    let bytes = file::inspect(path, 32 * 1024 * 1024, true, cancel)?.bytes;
    let config: Configuration = serde_json::from_slice(&bytes).map_err(invalid)?;
    if config.format_version != 1 || config.profiles.is_empty() || config.profiles.len() > 32 {
        return Err(invalid("SBV2設定の版またはプロファイル数が不正です"));
    }
    let mut ids = BTreeSet::new();
    for profile in &config.profiles {
        if !profile_id(&profile.id)
            || !ids.insert(&profile.id)
            || profile.name.trim().is_empty()
            || profile.name.len() > 256
            || profile.name.chars().any(char::is_control)
        {
            return Err(invalid("SBV2設定のID・名前・重複が不正です"));
        }
        profile.binding.managed_address()?;
        Catalog::parse(profile.catalog.clone()).map_err(invalid)?;
    }
    Ok(config.profiles)
}
impl Profile {
    /// Full offline validation for setup/build callers. Public single-speech
    /// starts capture first and perform the expensive inspection in the job.
    pub fn prepare(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        cancel: &AtomicBool,
    ) -> Result<Sbv2JobInput, ApiError> {
        let input = self.capture(revision, project, clip_id, cancel)?;
        let prepared = input.prepared().map_err(invalid)?;
        self.binding.inspect_selected(
            input.endpoint(),
            &input.catalog().map_err(invalid)?,
            prepared.audio().settings(),
            input.identity(),
            cancel,
        )?;
        Ok(input)
    }
    /// Capture bounded configuration and the saved identity, not multi-GB
    /// originals. The worker MUST verify every original before spawning.
    pub fn capture(
        &self,
        revision: u32,
        project: &Project,
        clip_id: &str,
        cancel: &AtomicBool,
    ) -> Result<Sbv2JobInput, ApiError> {
        let clip = project
            .clips
            .iter()
            .find(|clip| clip.id == clip_id)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "音声クリップが見つかりません"))?;
        let ClipContent::Speech { character_id, .. } = &clip.content else {
            return Err(invalid("SBV2の音声クリップを指定してください"));
        };
        let character = project
            .characters
            .iter()
            .find(|c| &c.id == character_id)
            .ok_or_else(|| invalid("音声キャラクターがありません"))?;
        let Voice::Sbv2 { settings, identity } = &character.voice else {
            return Err(invalid("キャラクターの音声エンジンがSBV2ではありません"));
        };
        let catalog = Catalog::parse(self.catalog.clone()).map_err(invalid)?;
        self.binding
            .validate_catalog(&self.binding.endpoint, &catalog, settings)?;
        let config = file::inspect(
            &self.binding.originals.config,
            4 * 1024 * 1024,
            true,
            cancel,
        )?;
        if config.sha256 != identity.config_sha256 {
            return Err(super::changed(
                "SBV2モデル設定が保存された識別情報と一致しません",
            ));
        }
        let config_text = String::from_utf8(config.bytes).map_err(invalid)?;
        Sbv2JobInput::new(
            revision,
            project,
            clip_id,
            &self.binding.endpoint,
            identity,
            &catalog,
            &config_text,
        )
        .map_err(invalid)
    }
}
