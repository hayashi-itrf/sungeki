use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetQwenProfileArgs {
    pub profile_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QwenProfileSummary {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QwenSpeaker {
    pub id: String,
    pub name: String,
}
/// Originals checked against the saved identity, not a synthesis success report.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct QwenProfileDetails {
    pub profile_id: String,
    pub name: String,
    pub identity: sungeki_project::QwenIdentity,
    pub speakers: Vec<QwenSpeaker>,
    pub languages: Vec<String>,
    pub default_settings: sungeki_project::QwenSettings,
}
