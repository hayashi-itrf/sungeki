use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckpointRequest {
    Delete {
        checkpoint_id: String,
    },
    Compare {
        checkpoint_id: String,
        target: CheckpointTarget,
        offset: u32,
        limit: u32,
    },
    Create {
        checkpoint_id: String,
        name: String,
        expected_revision: u32,
    },
    List {
        after: Option<String>,
        limit: u32,
    },
    Restore {
        checkpoint_id: String,
        operation_id: String,
        expected_revision: u32,
    },
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateCheckpointArgs {
    pub session_id: Option<String>,
    pub checkpoint_id: String,
    pub name: String,
    pub expected_revision: u32,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ListCheckpointsArgs {
    pub session_id: Option<String>,
    pub after: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RestoreCheckpointArgs {
    pub session_id: Option<String>,
    pub checkpoint_id: String,
    pub operation_id: String,
    pub expected_revision: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckpointTarget {
    Current { expected_revision: u32 },
    Checkpoint { checkpoint_id: String },
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CompareCheckpointArgs {
    pub session_id: Option<String>,
    pub checkpoint_id: String,
    pub target: CheckpointTarget,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
pub struct CheckpointComparison {
    pub checkpoint_id: String,
    pub target_checkpoint_id: Option<String>,
    pub target_revision: Option<u32>,
    pub before_hash: String,
    pub after_hash: String,
    pub before_duration_us: sungeki_project::TimeUs,
    pub after_duration_us: sungeki_project::TimeUs,
    pub total_changes: u32,
    pub entries: Vec<sungeki_editor::ProjectDifference>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DeleteCheckpointArgs {
    pub session_id: Option<String>,
    pub checkpoint_id: String,
}
