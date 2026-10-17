use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Succeeded,
    Failed,
    Interrupted,
}
impl JobPhase {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Succeeded | Self::Failed | Self::Interrupted
        )
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Progress {
    pub stage: String,
    pub fraction: Option<f64>,
}
