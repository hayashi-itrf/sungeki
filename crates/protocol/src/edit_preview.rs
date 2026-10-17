use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_editor::ProjectDifference;
use sungeki_project::TimeUs;
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct EditPreview {
    pub source_revision: u32,
    pub before_hash: String,
    pub after_hash: String,
    pub before_duration_us: TimeUs,
    pub after_duration_us: TimeUs,
    pub no_op: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub created_ids: Option<std::collections::BTreeMap<String, String>>,
    pub total_changes: u32,
    pub entries: Vec<ProjectDifference>,
    pub next_offset: Option<u32>,
}
