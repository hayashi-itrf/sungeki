use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VoicevoxEngineInfo {
    pub engine_id: String,
    pub name: String,
    pub version: String,
    pub core_versions: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VoicevoxStyleInfo {
    pub id: u32,
    pub name: String,
    pub kind: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VoicevoxSpeakerInfo {
    pub speaker_uuid: String,
    pub name: String,
    pub version: String,
    pub styles: Vec<VoicevoxStyleInfo>,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DiscoverVoicevoxArgs {
    pub base_url: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetVoicevoxCatalogArgs {
    pub base_url: String,
    pub expected_engine: VoicevoxEngineInfo,
    pub core_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionRequest;
    #[test]
    fn catalog_requires_minor_23_and_rejects_unknown_arguments() {
        assert_eq!(
            SessionRequest::DiscoverVoicevox {
                args: DiscoverVoicevoxArgs {
                    base_url: "http://127.0.0.1:50021".into()
                }
            }
            .minimum_protocol_minor(),
            23
        );
        assert_eq!(
            SessionRequest::GetVoicevoxCatalog {
                args: GetVoicevoxCatalogArgs {
                    base_url: "http://127.0.0.1:50021".into(),
                    core_version: "0.16.0".into(),
                    expected_engine: VoicevoxEngineInfo {
                        engine_id: "id".into(),
                        name: "engine".into(),
                        version: "1".into(),
                        core_versions: vec![]
                    }
                }
            }
            .minimum_protocol_minor(),
            23
        );
        assert!(
            serde_json::from_str::<DiscoverVoicevoxArgs>(
                r#"{"base_url":"http://127.0.0.1:50021","start_engine":true}"#
            )
            .is_err()
        );
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GetVoicevoxSpeakerArgs {
    pub base_url: String,
    pub expected_engine: VoicevoxEngineInfo,
    pub core_version: String,
    pub speaker_uuid: String,
}
#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VoicevoxSpeakerDetails {
    pub engine: VoicevoxEngineInfo,
    pub core_version: String,
    pub speaker: VoicevoxSpeakerInfo,
    pub engine_terms: String,
    pub speaker_policy: String,
    pub default_sampling_rate: u32,
}
