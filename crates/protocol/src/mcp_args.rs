use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_editor::Command;
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VideoPreviewArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proxy_job_id: Option<String>,
    pub session_id: String,
    pub expected_revision: u32,
    pub clip_id: String,
    pub time_us: sungeki_project::TimeUs,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EmptyArgs {}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PsdPreviewArgs {
    pub session_id: String,
    pub expected_revision: u32,
    pub clip_id: String,
    pub time_us: sungeki_project::TimeUs,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectArgs {
    pub title: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AttachSessionArgs {
    pub session_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetProjectArgs {
    pub session_id: Option<String>,
    pub view: Option<crate::ProjectView>,
    pub expected_revision: Option<u32>,
    pub scene_ids: Option<Vec<String>>,
    pub clip_ids: Option<Vec<String>>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ApplyCommandsArgs {
    pub session_id: Option<String>,
    pub expected_revision: Option<u32>,
    pub operation_id: Option<String>,
    pub commands: Vec<Command>,
    pub dry_run: Option<bool>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryArgs {
    pub session_id: Option<String>,
    pub expected_revision: Option<u32>,
    pub operation_id: String,
    pub expected_operation_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SaveProjectArgs {
    pub session_id: Option<String>,
    pub path: Option<String>,
    pub expected_revision: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OpenProjectArgs {
    pub path: String,
    #[serde(default)]
    pub import_checkpoints: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct ProbeImageArgs {
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct ProbeAudioArgs {
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct ProbeVideoArgs {
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct ProbeVrmArgs {
    pub path: String,
    pub kind: sungeki_project::VrmKind,
}
