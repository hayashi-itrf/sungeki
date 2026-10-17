use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ValidationFailed,
    Conflict,
    NotFound,
    UnsupportedSchema,
    AssetMissing,
    CapabilityUnavailable,
    EngineUnavailable,
    OutputExists,
    ExternalModification,
    Cancelled,
    IoError,
    ProtocolMismatch,
    HostUnavailable,
    InvalidRuntime,
    SidecarMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: BTreeMap<String, String>,
    pub retryable: bool,
    pub suggested_actions: Vec<String>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: BTreeMap::new(),
            retryable: false,
            suggested_actions: Vec::new(),
        }
    }
    pub fn io(error: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::IoError, error.to_string())
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for ApiError {}
