use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::JobPhase;
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectProjectArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub destination: String,
    #[serde(default)]
    pub include_checkpoints: bool,
    #[serde(default)]
    pub dry_run: bool,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollectionRequest {
    ReadPlan {
        args: crate::ReadCollectionPlanArgs,
    },
    Start {
        args: CollectProjectArgs,
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
#[serde(deny_unknown_fields)]
pub struct CollectionOutput {
    pub directory: String,
    pub project_path: String,
    pub manifest_path: String,
    pub collected_project_id: String,
    pub checkpoints_included: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CollectionJob {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub destination: String,
    pub phase: JobPhase,
    pub dry_run: bool,
    pub include_checkpoints: bool,
    pub checkpoint_count: u32,
    pub plan: Option<crate::CollectionPlanSummary>,
    pub output: Option<CollectionOutput>,
    pub error: Option<ApiError>,
    pub stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collection_is_versioned_and_unknown_arguments_are_rejected() {
        let request = crate::SessionRequest::Collection {
            request: CollectionRequest::Get {
                job_id: "fixture".into(),
                wait_ms: 0,
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 143);
        let mut args = serde_json::json!({"session_id":"s", "job_id":"j", "expected_revision":0, "destination":"/tmp/test"});
        let parsed: CollectProjectArgs = serde_json::from_value(args.clone()).unwrap();
        assert!(!parsed.include_checkpoints && !parsed.dry_run);
        args["silently_ignore_missing_assets"] = true.into();
        assert!(serde_json::from_value::<CollectProjectArgs>(args).is_err());
    }
}
