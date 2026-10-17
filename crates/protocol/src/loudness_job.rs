use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{JobPhase, Progress};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartLoudnessArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub asset_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LoudnessJobRequest {
    Start {
        args: StartLoudnessArgs,
    },
    Get {
        job_id: String,
        #[serde(default)]
        wait_ms: u32,
    },
    Cancel {
        job_id: String,
    },
    List {
        session_id: Option<String>,
        after: Option<String>,
        limit: u32,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum LoudnessMeasurement {
    Measured {
        integrated_lufs: f64,
        true_peak_db: f64,
        range_lu: f64,
        threshold_lufs: f64,
    },
    NotMeasurable {
        reason: LoudnessUnavailableReason,
        true_peak_db: Option<f64>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum LoudnessUnavailableReason {
    TooShort,
    BelowGateOrSilence,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LoudnessReport {
    pub version: u32,
    pub method: String,
    pub source_sha256: String,
    #[ts(type = "number")]
    pub source_frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub source_pre_gain: bool,
    pub dual_mono: bool,
    pub measurement: LoudnessMeasurement,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LoudnessJob {
    pub job_id: String,
    pub session_id: String,
    pub asset_id: String,
    pub source_revision: u32,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<LoudnessReport>,
    pub error: Option<ApiError>,
    // Revision freshness only; the source file is verified during execution.
    pub stale: bool,
}
