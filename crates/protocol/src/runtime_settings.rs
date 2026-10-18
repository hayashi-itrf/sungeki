use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(
    Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS,
)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSettings {
    pub format_version: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub voicevox_engine: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub aivis_engine: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub coeiroink_engine: Option<PathBuf>,
    pub runtime_root: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub font: Option<PathBuf>,
    pub font_sha256: Option<String>,
    pub vrm_node: Option<PathBuf>,
    pub vrm_render_worker: Option<PathBuf>,
    pub vrm_inspect_worker: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct SaveRuntimeSettingsArgs {
    pub expected: RuntimeSettings,
    pub settings: RuntimeSettings,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionRequest;
    #[test]
    fn runtime_settings_requests_require_protocol_151() {
        assert_eq!(
            SessionRequest::GetRuntimeSettings.minimum_protocol_minor(),
            151
        );
        assert_eq!(
            SessionRequest::SaveRuntimeSettings {
                args: SaveRuntimeSettingsArgs {
                    expected: RuntimeSettings::default(),
                    settings: RuntimeSettings::default()
                }
            }
            .minimum_protocol_minor(),
            151
        );
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ManagedSpeechEngine {
    Sbv2,
    Qwen,
    Irodori,
}
impl ManagedSpeechEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sbv2 => "sbv2",
            Self::Qwen => "qwen",
            Self::Irodori => "irodori",
        }
    }
}
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
pub struct InstallSpeechConfigArgs {
    pub engine: ManagedSpeechEngine,
    pub candidate_path: PathBuf,
}

#[cfg(test)]
mod installation_tests {
    use super::*;
    use crate::SessionRequest;
    #[test]
    fn installation_requires_new_protocol_and_explicit_engine() {
        let request = SessionRequest::InstallSpeechConfig {
            args: InstallSpeechConfigArgs {
                engine: ManagedSpeechEngine::Qwen,
                candidate_path: "/candidate.json".into(),
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 119);
        let encoded = serde_json::to_value(&request).unwrap();
        assert_eq!(encoded["args"]["engine"], "qwen");
        for args in [
            serde_json::json!({"engine":"other","candidate_path":"/candidate.json"}),
            serde_json::json!({"engine":"qwen","candidate_path":"/candidate.json","overwrite":true}),
        ] {
            assert!(serde_json::from_value::<InstallSpeechConfigArgs>(args).is_err());
        }
    }
}
