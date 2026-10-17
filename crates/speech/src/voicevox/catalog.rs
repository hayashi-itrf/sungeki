//! Read-only discovery. Metadata equality is not proof of process identity;
//! the execution lease owner must verify the engine again before synthesis.
use super::{
    Stage, TransportError,
    http::{check_cancel, connection, failure, read},
};
use reqwest::{Client, Url};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashSet, sync::atomic::AtomicBool, time::Duration};

const MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineInfo {
    pub engine_id: String,
    pub name: String,
    pub version: String,
    pub core_versions: Vec<String>,
}
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SpeakerStyle {
    pub name: String,
    pub id: u32,
    #[serde(rename = "type", default = "talk")]
    pub kind: String,
}
fn talk() -> String {
    "talk".into()
}
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Speaker {
    pub name: String,
    pub speaker_uuid: String,
    pub version: String,
    pub styles: Vec<SpeakerStyle>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCatalog {
    pub engine: EngineInfo,
    pub core_version: String,
    pub speakers: Vec<Speaker>,
}
#[derive(Deserialize)]
struct Manifest {
    uuid: String,
    name: String,
}

pub struct VoicevoxDiscovery {
    client: Client,
    base: Url,
}
impl VoicevoxDiscovery {
    pub fn new(base: &str, timeout: Duration) -> Result<Self, TransportError> {
        let (client, base) = connection(base, timeout)?;
        Ok(Self { client, base })
    }
    pub async fn engine(&self, cancelled: &AtomicBool) -> Result<EngineInfo, TransportError> {
        let manifest: Manifest = self.get("engine_manifest", None, cancelled).await?;
        identifier(&manifest.uuid)?;
        label(&manifest.name, 1024)?;
        let version: String = self.get("version", None, cancelled).await?;
        label(&version, 128)?;
        let core_versions: Vec<String> = self.get("core_versions", None, cancelled).await?;
        if core_versions.is_empty() || core_versions.len() > 128 {
            return Err(invalid("利用可能なcore一覧が空、または多すぎます"));
        }
        let mut seen = HashSet::new();
        for core in &core_versions {
            label(core, 128)?;
            if !seen.insert(core) {
                return Err(invalid("core一覧に重複があります"));
            }
        }
        Ok(EngineInfo {
            engine_id: manifest.uuid,
            name: manifest.name,
            version,
            core_versions,
        })
    }
    /// Caller chooses an explicitly advertised core; no guessed latest/default.
    /// A failed/changed refresh returns no partial catalog or cached fallback.
    pub async fn catalog(
        &self,
        expected: &EngineInfo,
        core: &str,
        cancelled: &AtomicBool,
    ) -> Result<VoiceCatalog, TransportError> {
        if !expected.core_versions.iter().any(|item| item == core) {
            return Err(invalid("一覧にないcoreは選択できません"));
        }
        let before = self.engine(cancelled).await?;
        if &before != expected {
            return Err(invalid("エンジン情報が変わりました。再取得してください"));
        }
        let mut speakers: Vec<Speaker> = self.get("speakers", Some(core), cancelled).await?;
        validate_speakers(&speakers)?;
        for speaker in &mut speakers {
            speaker
                .styles
                .retain(|style| matches!(style.kind.as_str(), "talk" | "streaming_talk"));
        }
        speakers.retain(|speaker| !speaker.styles.is_empty());
        if self.engine(cancelled).await? != before {
            return Err(invalid("話者取得中にエンジン情報が変わりました"));
        }
        Ok(VoiceCatalog {
            engine: before,
            core_version: core.into(),
            speakers,
        })
    }
    pub(super) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        core: Option<&str>,
        cancelled: &AtomicBool,
    ) -> Result<T, TransportError> {
        let pairs: Vec<_> = core
            .map(|core| vec![("core_version", core)])
            .unwrap_or_default();
        self.get_pairs(path, &pairs, cancelled).await
    }
    pub(super) async fn get_pairs<T: DeserializeOwned>(
        &self,
        path: &str,
        pairs: &[(&str, &str)],
        cancelled: &AtomicBool,
    ) -> Result<T, TransportError> {
        check_cancel(cancelled)?;
        let mut url = self.base.clone();
        url.set_path(path);
        if !pairs.is_empty() {
            url.query_pairs_mut().extend_pairs(pairs.iter().copied());
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| failure(Stage::Catalog, e))?;
        let bytes = read(response, Stage::Catalog, MAX_METADATA_BYTES).await?;
        check_cancel(cancelled)?;
        serde_json::from_slice(&bytes).map_err(|_| invalid("VOICEVOXの一覧応答が不正です"))
    }
}
fn validate_speakers(speakers: &[Speaker]) -> Result<(), TransportError> {
    if speakers.len() > 4096 {
        return Err(invalid("話者が多すぎます"));
    }
    let mut ids = HashSet::new();
    let mut styles = HashSet::new();
    for speaker in speakers {
        identifier(&speaker.speaker_uuid)?;
        label(&speaker.name, 1024)?;
        label(&speaker.version, 128)?;
        if !ids.insert(speaker.speaker_uuid.to_ascii_lowercase()) {
            return Err(invalid("話者IDに重複があります"));
        }
        for style in &speaker.styles {
            label(&style.name, 1024)?;
            label(&style.kind, 128)?;
            if !styles.insert(style.id) {
                return Err(invalid("スタイルIDに重複があります"));
            }
            if styles.len() > 16384 {
                return Err(invalid("スタイルが多すぎます"));
            }
        }
    }
    Ok(())
}
fn label(value: &str, limit: usize) -> Result<(), TransportError> {
    if value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(invalid("一覧に空・長すぎる・制御文字入りの項目があります"));
    }
    Ok(())
}
pub(super) fn identifier(value: &str) -> Result<(), TransportError> {
    // Canonical hyphenated UUID, accepted case-insensitively but not normalized.
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(invalid("一覧のUUIDが不正です"));
    }
    Ok(())
}
pub(super) fn invalid(message: &str) -> TransportError {
    TransportError::Metadata(message.into())
}
