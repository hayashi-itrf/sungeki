use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetSbv2ProfileArgs {
    pub profile_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2Speaker {
    pub id: u32,
    pub name: String,
}
/// Measured configured originals, not an assertion that synthesis was tested.
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2ProfileDetails {
    pub profile_id: String,
    pub name: String,
    pub identity: sungeki_project::Sbv2Identity,
    pub speakers: Vec<Sbv2Speaker>,
    pub styles: Vec<String>,
    pub languages: Vec<sungeki_project::Sbv2Language>,
    pub default_settings: sungeki_project::Sbv2Settings,
}
