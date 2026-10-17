use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleFormat {
    Srt,
    WebVtt,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ExportSubtitlesArgs {
    pub session_id: String,
    pub expected_revision: u32,
    pub path: String,
    pub format: SubtitleFormat,
    #[serde(default)]
    pub include_text: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SubtitlePublication {
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub path: String,
    pub format: SubtitleFormat,
    pub include_text: bool,
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub already_present: bool,
    /// Whether the project changed between snapshot acquisition and the response check.
    pub stale: bool,
}
