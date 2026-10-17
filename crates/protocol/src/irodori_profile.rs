use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetIrodoriProfileArgs {
    pub profile_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IrodoriProfileSummary {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IrodoriSpeaker {
    pub id: String,
    pub name: String,
}
/// Originals checked against the saved identity, not a synthesis success report.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IrodoriProfileDetails {
    pub profile_id: String,
    pub name: String,
    pub identity: sungeki_project::IrodoriIdentity,
    pub speakers: Vec<IrodoriSpeaker>,
    pub speaker_conditioning: bool,
    pub caption_conditioning: bool,
    pub default_settings: sungeki_project::IrodoriSettings,
}
