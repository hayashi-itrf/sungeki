use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProposeSpeechGainArgs {
    pub session_id: String,
    pub expected_revision: u32,
    pub job_id: String,
    pub clip_id: String,
    pub target_lufs: f64,
    pub max_true_peak_db: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechGainEstimate {
    pub target_lufs: f64,
    pub max_true_peak_db: f64,
    pub requested_gain_db: f64,
    pub gain_db: f64,
    pub predicted_lufs: f64,
    pub predicted_true_peak_db: f64,
    pub peak_margin_db: f64,
    pub limited_by_peak: bool,
    pub limited_by_gain_range: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechGainProposal {
    pub session_id: String,
    pub source_revision: u32,
    pub job_id: String,
    pub clip_id: String,
    pub asset_id: String,
    pub source_sha256: String,
    pub current_gain_db: Option<f64>,
    pub estimate: SpeechGainEstimate,
    // Apply via the shared editor at source_revision. Replaces existing gain.
    pub command: sungeki_editor::Command,
}
