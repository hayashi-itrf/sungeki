use crate::{ApiError, AssetFreshness, RenderOutput, ReviewReport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{BuildRecipe, JobPhase, Progress};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartBuildVideoArgs {
    /// Response waiting only; does not participate in the durable recipe identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    /// Explicit continuation: new speech index -> successful source speech index (both zero-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reuse_from: Option<BuildReuseRequest>,
    pub job_id: String,
    pub recipe: BuildRecipe,
    pub project_path: String,
    /// VOICEVOX connection; required when the recipe contains VOICEVOX roles.
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub engine_version: String,
    /// Managed engine profile IDs keyed by character ID (not role name).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub managed_profiles: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildReuseRequest {
    pub parent_job_id: String,
    pub speech: std::collections::BTreeMap<String, u16>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildReuseSource {
    /// Recorded identity for reuse without live discovery; Start still verifies originals.
    pub base_url: String,
    pub engine_version: String,
    pub job_id: String,
    pub title: String,
    pub lines: Vec<BuildReuseLine>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildReuseLine {
    pub index: u16,
    pub character_id: String,
    pub character_name: String,
    pub subtitle_text: String,
    pub spoken_text: Option<String>,
    /// Journal evidence only; Start rechecks originals and audio settings.
    pub has_saved_audio: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReconcileBuildAudioArgs {
    pub job_id: String,
    pub child_job_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BuildVideoRequest {
    Start {
        args: StartBuildVideoArgs,
    },
    ReconcileAudio {
        args: ReconcileBuildAudioArgs,
    },
    ReadReuseSource {
        job_id: String,
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
        after: Option<String>,
        limit: u32,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildVideoOutput {
    pub project_path: String,
    pub project_sha256: String,
    pub render_job_id: String,
    pub render: RenderOutput,
    pub review_job_id: String,
    pub review: ReviewReport,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct BuildVideoJob {
    pub job_id: String,
    pub session_id: String,
    pub title: String,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub completed_steps: u32,
    pub total_steps: u32,
    pub child_job_id: Option<String>,
    pub source_revision: u32,
    pub output: Option<BuildVideoOutput>,
    pub error: Option<ApiError>,
    /// Editing freshness only; Get does not reverify media bytes or quality.
    pub stale: bool,
    pub review_asset_freshness: Option<AssetFreshness>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_profiles_are_explicit_and_require_new_protocol() {
        let mut value = serde_json::json!({
            "job_id":"test", "project_path":"/tmp/video.vvm.json",
            "recipe":serde_json::from_str::<serde_json::Value>(include_str!("../../../tests/fixtures/recipes/build-dialogue.json")).unwrap(),
            "managed_profiles":{"character-id":"local-profile"}
        });
        let args: StartBuildVideoArgs = serde_json::from_value(value.clone()).unwrap();
        assert!(args.base_url.is_empty());
        assert!(args.engine_version.is_empty());
        let request = crate::SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start { args },
        };
        assert_eq!(request.minimum_protocol_minor(), 106);
        value["managed_profiles"] = serde_json::json!({"character-id":42});
        assert!(serde_json::from_value::<StartBuildVideoArgs>(value).is_err());
    }
}

#[cfg(test)]
mod reuse_tests {
    #[test]
    fn reuse_requires_minor_123_and_rejects_unknown_fields() {
        let recipe: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/recipes/build-dialogue.json"
        ))
        .unwrap();
        let mut value = serde_json::json!({"type":"build_video","request":{"type":"start","args":{
            "job_id":"new", "project_path":"/tmp/new.vvm.json", "recipe":recipe,
            "reuse_from":{"parent_job_id":"old","speech":{"0":1}}
        }}});
        let request: crate::SessionRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 123);
        value["request"]["args"]["reuse_from"]["automatic"] = serde_json::json!(true);
        assert!(serde_json::from_value::<crate::SessionRequest>(value).is_err());
    }
}

#[cfg(test)]
mod recovery_tests {
    #[test]
    fn explicit_reconciliation_requires_both_ids_and_protocol_124() {
        let value = serde_json::json!({"type":"build_video","request":{
            "type":"reconcile_audio", "args":{"job_id":"p", "child_job_id":"c"}
        }});
        let request: crate::SessionRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 124);
        let mut missing = value.clone();
        missing["request"]["args"]
            .as_object_mut()
            .unwrap()
            .remove("child_job_id");
        assert!(serde_json::from_value::<crate::SessionRequest>(missing).is_err());
        let mut unknown = value;
        unknown["request"]["args"]["restart"] = true.into();
        assert!(serde_json::from_value::<crate::SessionRequest>(unknown).is_err());
    }
}

#[cfg(test)]
mod source_tests {
    #[test]
    fn reading_source_requires_126() {
        assert_eq!(
            crate::SessionRequest::BuildVideo {
                request: super::BuildVideoRequest::ReadReuseSource {
                    job_id: "parent".into()
                }
            }
            .minimum_protocol_minor(),
            126
        );
    }
}
