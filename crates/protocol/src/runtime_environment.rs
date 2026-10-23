use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ts_rs::TS;

/// Selection diagnostics for future operations, not a job's frozen runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEnvironment {
    pub data_directory: PathBuf,
    pub runtime_settings_path: PathBuf,
    pub scope: RuntimeEnvironmentScope,
    pub saved: RuntimeEnvironmentValues,
    pub selected: RuntimeEnvironmentSelections,
    pub verification: RuntimeEnvironmentVerification,
}

/// VRM, speech profiles, per-operation work directories and running jobs are excluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEnvironmentScope {
    RenderPathsAndRuntimeRoot,
}

/// Referenced files, mounts, hashes, executables and rendering are not checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEnvironmentVerification {
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEnvironmentValues {
    pub runtime_root: Option<String>,
    pub ffmpeg: Option<String>,
    pub ffprobe: Option<String>,
    pub font: Option<String>,
    pub font_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEnvironmentSelections {
    pub runtime_root: RuntimeSettingSelection,
    pub ffmpeg: RuntimeSettingSelection,
    pub ffprobe: RuntimeSettingSelection,
    pub font: RuntimeSettingSelection,
    pub font_sha256: RuntimeSettingSelection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSettingSelection {
    /// Selected verbatim, including an empty or invalid explicit override.
    pub value: Option<String>,
    pub source: RuntimeSettingSource,
    pub environment_variable: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSettingSource {
    Environment,
    Saved,
    Unset,
}

#[cfg(test)]
mod tests {
    use crate::SessionRequest;

    #[test]
    fn environment_read_requires_new_protocol_and_no_arguments() {
        let request = SessionRequest::GetRuntimeEnvironment {};
        assert_eq!(request.minimum_protocol_minor(), 160);
        assert_eq!(
            SessionRequest::GetRuntimeSettings.minimum_protocol_minor(),
            151
        );
        assert_eq!(
            serde_json::to_value(request).unwrap()["type"],
            "get_runtime_environment"
        );
        assert!(
            serde_json::from_value::<SessionRequest>(serde_json::json!({
                "type": "get_runtime_environment", "path": "/other/runtime.json"
            }))
            .is_err()
        );
    }
}
