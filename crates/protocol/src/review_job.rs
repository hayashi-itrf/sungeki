use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{Diagnostics, JobPhase, Progress};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewReport {
    pub report_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    /// None means a complete image fingerprint was not obtained (uninspected or failed).
    pub asset_fingerprint: Option<String>,
    pub diagnostics: Diagnostics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub representative_frames: Option<crate::ReviewFrames>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewJob {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub report: Option<ReviewReport>,
    pub error: Option<ApiError>,
    /// Project changes or a source mismatch found by the latest background scan.
    /// False is not a freshness guarantee; inspect asset_freshness as well.
    pub stale: bool,
    pub asset_freshness: AssetFreshness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AssetFreshnessStatus {
    Unchecked,
    Matching,
    Changed,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AssetFreshness {
    pub status: AssetFreshnessStatus,
    /// Retains a known mismatch through subsequent read failures; a matching scan clears it.
    pub last_known_changed: bool,
    pub checking: bool,
    /// Completion time of the most recent observation, not a filesystem snapshot time.
    #[ts(type = "number | null")]
    pub checked_at_ms: Option<u64>,
    pub message: Option<String>,
}
impl Default for AssetFreshness {
    fn default() -> Self {
        Self {
            status: AssetFreshnessStatus::Unchecked,
            last_known_changed: false,
            checking: false,
            checked_at_ms: None,
            message: None,
        }
    }
}
