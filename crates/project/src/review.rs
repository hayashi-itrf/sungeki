use crate::TimeUs;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotRun,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    Structure,
    AssetIntegrity,
    TextLayout,
    Audio,
    RepresentativeFrames,
    VisualReview,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Check {
    pub kind: CheckKind,
    pub status: CheckStatus,
    pub description: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Issue {
    pub severity: Severity,
    pub code: String,
    pub field: String,
    pub clip_id: Option<String>,
    pub scene_id: Option<String>,
    pub asset_id: Option<String>,
    pub start_us: Option<TimeUs>,
    pub end_us: Option<TimeUs>,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Diagnostics {
    pub checks: Vec<Check>,
    pub issues: Vec<Issue>,
    /// This is a count of tests, not a claim about coverage of the whole video.
    pub summary: Summary,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct Summary {
    pub passed: usize,
    pub failed: usize,
    pub not_run: usize,
    pub issue_count: usize,
    pub complete: bool,
}
impl Diagnostics {
    pub fn new(checks: Vec<Check>, issues: Vec<Issue>) -> Self {
        let count = |status| checks.iter().filter(|c| c.status == status).count();
        let not_run = count(CheckStatus::NotRun);
        let summary = Summary {
            passed: count(CheckStatus::Passed),
            failed: count(CheckStatus::Failed),
            not_run,
            issue_count: issues.len(),
            complete: not_run == 0,
        };
        Self {
            checks,
            issues,
            summary,
        }
    }
}
