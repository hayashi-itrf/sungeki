use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{JobPhase, Progress};
use ts_rs::TS;
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FramePreviewRequest {
    Read {
        args: ReadFramePreviewArgs,
    },
    Publish {
        args: PublishFramePreviewArgs,
    },
    Start {
        args: StartFramePreviewArgs,
    },
    Get {
        job_id: String,
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
pub struct FramePreviewOutput {
    pub path: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    #[ts(type = "number")]
    pub frame_index: u64,
    pub time_us: sungeki_project::TimeUs,
    pub font_sha256: String,
    pub layout_version: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FramePreviewJob {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<FramePreviewOutput>,
    pub error: Option<ApiError>,
    pub stale: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartFramePreviewArgs {
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
    #[ts(type = "number")]
    pub frame_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadFramePreviewArgs {
    pub job_id: String,
    #[ts(type = "number")]
    pub offset: u64,
    pub length: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FramePreviewBytes {
    #[ts(type = "number")]
    pub offset: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub base64: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PublishFramePreviewArgs {
    pub job_id: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct FramePreviewPublication {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub output: FramePreviewOutput,
    pub already_present: bool,
    pub stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_requires_minor72_and_does_not_accept_a_read_path() {
        let request = crate::SessionRequest::FramePreview {
            request: FramePreviewRequest::Read {
                args: ReadFramePreviewArgs {
                    job_id: "j".into(),
                    offset: 0,
                    length: 1,
                },
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 72);
        assert!(
            serde_json::from_value::<ReadFramePreviewArgs>(serde_json::json!({
                "job_id": "j", "offset": 0, "length": 1, "path": "/arbitrary/file"
            }))
            .is_err()
        );
    }
}
