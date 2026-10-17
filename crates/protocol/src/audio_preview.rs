use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{JobPhase, Progress};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StartAudioPreviewArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub session_id: String,
    pub job_id: String,
    pub expected_revision: u32,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioPreviewRequest {
    Start {
        args: StartAudioPreviewArgs,
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
    Read {
        args: ReadAudioPreviewArgs,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadAudioPreviewArgs {
    pub job_id: String,
    #[ts(type = "number")]
    pub offset: u64,
    pub length: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioPreviewOutput {
    pub path: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    #[ts(type = "number")]
    pub sample_frames: u64,
    pub sample_rate: u32,
    pub normalization: Option<sungeki_project::Normalization>,
    pub chunk_sha256: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioPreviewJob {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub output: Option<AudioPreviewOutput>,
    pub error: Option<ApiError>,
    pub stale: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioPreviewBytes {
    #[ts(type = "number")]
    pub offset: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub base64: String,
}

#[cfg(test)]
mod tests {

    #[test]
    fn audio_preview_requires_minor_44() {
        assert_eq!(
            crate::SessionRequest::AudioPreview {
                request: crate::AudioPreviewRequest::Read {
                    args: crate::ReadAudioPreviewArgs {
                        job_id: "fixture".into(),
                        offset: 0,
                        length: 1,
                    }
                },
            }
            .minimum_protocol_minor(),
            44
        );
    }
}
