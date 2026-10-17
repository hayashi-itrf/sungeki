use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EventCursor {
    pub host_instance_id: String,
    pub event_seq: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventKind {
    CheckpointCreated,
    CheckpointDeleted,
    Created,
    Edited,
    Saved,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SessionEvent {
    pub event_seq: u32,
    pub session_id: String,
    pub revision: u32,
    #[serde(rename = "type")]
    pub kind: SessionEventKind,
    pub payload: EventPayload,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EventBatch {
    pub cursor: EventCursor,
    pub events: Vec<SessionEvent>,
    pub resync_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EventPayload {
    pub operation_id: Option<String>,
}
