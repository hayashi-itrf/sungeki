use crate::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sungeki_project::{JobPhase, Progress};
use ts_rs::TS;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProxyPreset {
    Compact,
    Standard,
}
impl ProxyPreset {
    pub fn max_edge(self) -> u32 {
        match self {
            Self::Compact => 640,
            Self::Standard => 1280,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GenerateProxiesArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait_ms: Option<u32>,
    pub job_id: String,
    pub session_id: String,
    pub expected_revision: u32,
    pub asset_ids: Vec<String>,
    pub preset: ProxyPreset,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProxyRequest {
    Remove {
        args: RemoveProxyArgs,
    },
    Start {
        args: GenerateProxiesArgs,
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
pub struct ProxyOutput {
    pub asset_id: String,
    pub key: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProxyJob {
    pub job_id: String,
    pub session_id: String,
    pub source_revision: u32,
    pub content_hash: String,
    pub asset_ids: Vec<String>,
    pub max_edge: u32,
    pub phase: JobPhase,
    pub progress: Option<Progress>,
    pub completed: Vec<ProxyOutput>,
    pub error: Option<ApiError>,
    /// Editing freshness only. Reading this job does not revalidate cache bytes.
    pub stale: bool,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_operations_require_expected_minor() {
        let request = crate::SessionRequest::Proxy {
            request: ProxyRequest::Get {
                job_id: "test".into(),
                wait_ms: 0,
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 142);
        let remove = crate::SessionRequest::Proxy {
            request: ProxyRequest::Remove {
                args: RemoveProxyArgs {
                    job_id: "job".into(),
                    asset_id: "asset".into(),
                },
            },
        };
        assert_eq!(remove.minimum_protocol_minor(), 89);
        assert_eq!(ProxyPreset::Compact.max_edge(), 640);
        assert_eq!(ProxyPreset::Standard.max_edge(), 1280);
        assert!(
            serde_json::from_str::<ProxyRequest>(
                r#"{"type":"get","job_id":"x","path":"/arbitrary"}"#
            )
            .is_err()
        );
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RemoveProxyArgs {
    pub job_id: String,
    pub asset_id: String,
}
