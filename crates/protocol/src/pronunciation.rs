use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PreviewPronunciationArgs {
    pub session_id: Option<String>,
    pub expected_revision: Option<u32>,
    pub subtitle_text: String,
    pub spoken_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PronunciationUse {
    pub written: String,
    pub spoken: String,
    pub occurrences: u32,
}
