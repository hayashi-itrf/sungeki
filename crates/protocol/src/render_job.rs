use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{JobPhase, Progress};
use ts_rs::TS;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RenderPurpose {
    #[default]
    Final,
    EditingPreview,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartRenderArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PublishRenderArgs {
    pub job_id: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RenderPublication {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub output: RenderOutput,
    pub already_present: bool,
    // Snapshot freshness at the start of this copy, not ongoing monitoring.
    pub stale: bool,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RenderJobRequest {
    Frame {
        args: RenderFrameArgs,
    },
    Preview {
        args: StartRenderArgs,
    },
    Read {
        args: ReadRenderArgs,
    },
    Publish {
        args: PublishRenderArgs,
    },
    Start {
        args: StartRenderArgs,
    },
    Get {
        job_id: String,
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
pub struct RenderFrameArgs {
    pub job_id: String,
    pub time_us: sungeki_project::TimeUs,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RenderFrame {
    pub job_id: String,
    pub time_us: sungeki_project::TimeUs,
    #[ts(type = "number")]
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
    pub png_base64: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RenderOutput {
    pub path: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    #[ts(type = "number")]
    pub frame_count: u64,
    pub font_sha256: String,
    pub layout_version: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RenderJob {
    pub purpose: RenderPurpose,
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<RenderOutput>,
    pub error: Option<ApiError>,
    // Project differs from the submitted snapshot. External assets are captured
    // during execution; false is not a new integrity scan of the output file.
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadRenderArgs {
    pub job_id: String,
    #[ts(type = "number")]
    pub offset: u64,
    pub length: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RenderBytes {
    #[ts(type = "number")]
    pub offset: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub base64: String,
}
#[cfg(test)]
mod playback_tests {
    use super::*;
    #[test]
    fn preview_requires_minor_111() {
        assert_eq!(
            crate::SessionRequest::RenderJob {
                request: RenderJobRequest::Preview {
                    args: StartRenderArgs {
                        wait_ms: None,
                        session_id: "s".into(),
                        job_id: "j".into(),
                        expected_revision: 0,
                    }
                }
            }
            .minimum_protocol_minor(),
            111
        );
    }
    #[test]
    fn render_bytes_require_minor_110() {
        assert_eq!(
            crate::SessionRequest::RenderJob {
                request: RenderJobRequest::Read {
                    args: ReadRenderArgs {
                        job_id: "fixture".into(),
                        offset: 0,
                        length: 1
                    }
                }
            }
            .minimum_protocol_minor(),
            110
        );
    }
}
