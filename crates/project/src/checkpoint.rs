use crate::{AssetLocator, Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CheckpointSummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    /// Unix milliseconds, bounded to JavaScript's safe integer range by storage.
    #[ts(type = "number")]
    pub created_at: u64,
    pub source_revision: u32,
    pub content_hash: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CheckpointAsset {
    pub asset_id: String,
    pub locator: AssetLocator,
    pub content_hash: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub summary: CheckpointSummary,
    pub project: Project,
    pub asset_manifest: Vec<CheckpointAsset>,
}
