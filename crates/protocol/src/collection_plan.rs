use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectionPlanSummary {
    #[serde(default)]
    pub checkpoint_count: u32,
    pub sources_ready: bool,
    pub destination_exists: bool,
    pub source_count: u32,
    pub unique_files: u32,
    /// Asset bytes only; excludes document/manifest and filesystem overhead.
    #[ts(type = "number")]
    pub estimated_asset_bytes: u64,
    pub excluded_dependencies: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CollectionSourceStatus {
    Verified,
    Missing,
    Changed,
    Unresolved,
    Unreadable,
    NotInspected,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectionPlanItem {
    pub asset_id: String,
    #[serde(default)]
    pub checkpoint_id: Option<String>,
    pub source_path: Option<String>,
    pub source_path_truncated: bool,
    pub destination_path: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub status: CollectionSourceStatus,
    pub detail: Option<String>,
    pub conflicting_metadata: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadCollectionPlanArgs {
    pub job_id: String,
    pub offset: u32,
    pub limit: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectionPlanPage {
    pub offset: u32,
    pub total: u32,
    pub items: Vec<CollectionPlanItem>,
}
