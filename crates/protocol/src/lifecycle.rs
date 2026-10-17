use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExitAction {
    Inspect,
    StopIfIdle,
    WhenIdle,
    CancelAndStop,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ExitStatus {
    pub active_jobs: u32,
    pub other_clients: u32,
    pub pending_requests: u32,
    pub stopping: bool,
}
