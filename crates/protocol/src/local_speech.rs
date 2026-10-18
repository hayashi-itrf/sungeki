use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LocalSpeechCatalogArgs {
    pub base_url: String,
    pub engine: sungeki_project::LocalSpeechEngine,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LocalSpeechCatalog {
    pub engine: sungeki_project::LocalSpeechEngine,
    pub version: String,
    pub speakers: Vec<LocalSpeechSpeaker>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LocalSpeechSpeaker {
    pub speaker_uuid: String,
    pub name: String,
    pub styles: Vec<LocalSpeechStyle>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LocalSpeechStyle {
    pub id: i32,
    pub name: String,
}

#[derive(Debug,Serialize,Deserialize,JsonSchema,TS)]
#[serde(deny_unknown_fields)]
pub struct LocalSpeechPolicyArgs {pub base_url:String,pub engine:sungeki_project::LocalSpeechEngine,pub speaker_uuid:String}
