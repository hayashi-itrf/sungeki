use schemars::JsonSchema;
use serde::Deserialize;
use ts_rs::TS;
#[derive(Debug, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewProjectArgs {
    pub sampling: Option<crate::ReviewFrameOptions>,
    pub session_id: Option<String>,
    pub job_id: String,
    pub expected_revision: Option<u32>,
    pub wait_ms: Option<u32>,
}
#[derive(Debug, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetJobArgs {
    pub job_id: String,
    pub wait_ms: Option<u32>,
}
#[derive(Debug, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CancelJobArgs {
    pub job_id: String,
}
#[derive(Debug, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ListJobsArgs {
    pub session_id: Option<String>,
    pub after: Option<String>,
    pub limit: Option<u32>,
}
