#[path = "irodori_profile.rs"]
mod irodori_profile;
pub use irodori_profile::*;
#[path = "qwen_profile.rs"]
mod qwen_profile;
pub use qwen_profile::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartSpeechArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub clip_id: String,
    pub base_url: String,
    /// Explicit version obtained from discovery. Rechecked before synthesis.
    pub engine_version: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartSbv2SpeechArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub clip_id: String,
    pub profile_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartQwenSpeechArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub clip_id: String,
    pub profile_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartIrodoriSpeechArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    pub clip_id: String,
    pub profile_id: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeechJobRequest {
    ListIrodoriProfiles,
    GetIrodoriProfile {
        args: crate::GetIrodoriProfileArgs,
    },
    StartIrodori {
        args: StartIrodoriSpeechArgs,
    },
    ListQwenProfiles,
    GetQwenProfile {
        args: crate::GetQwenProfileArgs,
    },
    StartQwen {
        args: StartQwenSpeechArgs,
    },
    ListSbv2Profiles,
    GetSbv2Profile {
        args: crate::GetSbv2ProfileArgs,
    },
    StartSbv2 {
        args: StartSbv2SpeechArgs,
    },
    Start {
        args: StartSpeechArgs,
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
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechArtifact {
    pub path: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub duration_us: u64,
    pub sample_rate: u32,
    pub channels: u16,
    #[ts(type = "number")]
    pub frames: u64,
    #[ts(type = "number")]
    pub byte_len: u64,
    pub lip_sync: Option<sungeki_project::LipSyncAnalysis>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeechApplication {
    NotApplicable,
    PendingConflict,
    PendingParent {
        parent_job_id: String,
    },
    Applied {
        revision: u32,
        overlapping_clip_ids: Vec<String>,
    },
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpeechJob {
    pub job_id: String,
    pub session_id: String,
    pub clip_id: String,
    pub source_revision: u32,
    pub input_hash: String,
    pub phase: sungeki_project::JobPhase,
    pub progress: Option<sungeki_project::Progress>,
    pub engine_settlement_required: bool,
    /// Target changed since capture. Does not attest current file/engine health.
    pub stale: bool,
    pub artifact: Option<SpeechArtifact>,
    pub application: SpeechApplication,
    pub error: Option<crate::ApiError>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Sbv2ProfileSummary {
    pub id: String,
    pub name: String,
    pub cpu_only: bool,
}

impl SpeechJobRequest {
    pub(crate) fn minimum_protocol_minor(&self) -> u16 {
        match self {
            Self::Start { args } if args.wait_ms.is_some() => 139,
            Self::StartSbv2 { args } if args.wait_ms.is_some() => 139,
            Self::StartQwen { args } if args.wait_ms.is_some() => 139,
            Self::StartIrodori { args } if args.wait_ms.is_some() => 139,
            Self::Get { .. } => 138,
            Self::ListIrodoriProfiles
            | Self::GetIrodoriProfile { .. }
            | Self::StartIrodori { .. } => 105,
            Self::ListQwenProfiles | Self::GetQwenProfile { .. } => 102,
            Self::StartQwen { .. } => 101,
            Self::GetSbv2Profile { .. } => 70,
            Self::ListSbv2Profiles => 69,
            Self::StartSbv2 { .. } => 68,
            _ => 32,
        }
    }
}

#[cfg(test)]
mod qwen_tests {
    use super::*;
    #[test]
    fn qwen_start_is_versioned_and_rejects_extra_launch_arguments() {
        let value = serde_json::json!({"type":"start_qwen","args":{
            "session_id":"session","job_id":"job","expected_revision":2,
            "clip_id":"clip","profile_id":"local"}});
        let request: SpeechJobRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 101);
        let mut invalid = value;
        invalid["args"]["base_url"] = serde_json::json!("http://elsewhere");
        assert!(serde_json::from_value::<SpeechJobRequest>(invalid).is_err());
    }
}

#[cfg(test)]
mod irodori_tests {
    use super::*;
    #[test]
    fn irodori_start_is_versioned_and_rejects_extra_launch_arguments() {
        let value = serde_json::json!({"type":"start_irodori","args":{
            "session_id":"session","job_id":"job","expected_revision":2,
            "clip_id":"clip","profile_id":"local"}});
        let request: SpeechJobRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 105);
        let mut invalid = value;
        invalid["args"]["base_url"] = serde_json::json!("http://elsewhere");
        assert!(serde_json::from_value::<SpeechJobRequest>(invalid).is_err());
    }
}
