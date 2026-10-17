use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// These typed collections are present from the first draft. Their production
// editing commands and richer engine-specific settings are introduced in M2/M3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub locator: AssetLocator,
    pub name: String,
    pub source: Option<String>,
    pub probe: AssetProbe,
    pub credit: String,
    pub content_hash: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssetLocator {
    ProjectRelative { path: String },
    Library { library_id: String, path: String },
    Absolute { path: String },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PronunciationEntry {
    pub written: String,
    pub spoken: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "type",
    content = "info",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AssetProbe {
    Psd(crate::PsdProbe),
    Image(crate::ImageProbe),
    Audio(crate::AudioProbe),
    Video(crate::VideoProbe),
    Vrm(crate::VrmAssetProbe),
}
impl AssetProbe {
    pub fn sha256(&self) -> &str {
        match self {
            Self::Psd(p) => &p.sha256,
            Self::Image(p) => &p.sha256,
            Self::Audio(p) => &p.sha256,
            Self::Video(p) => &p.sha256,
            Self::Vrm(p) => &p.sha256,
        }
    }
}
