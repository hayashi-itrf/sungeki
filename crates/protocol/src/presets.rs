use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use sungeki_project::Preset;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct InstantiatePresetArgs {
    pub session_id: String,
    pub operation_id: String,
    pub expected_revision: u32,
    pub preset_id: String,
    pub preset_revision: u32,
    pub ids: sungeki_editor::CharacterPresetIds,
    pub placement: Option<sungeki_editor::PresetStandingPlacement>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresetRequest {
    List {
        after_id: Option<String>,
        limit: u32,
    },
    Get {
        id: String,
    },
    Save {
        id: String,
        expected_revision: u32,
        preset: Preset,
    },
    Delete {
        id: String,
        expected_revision: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PresetRecord {
    pub id: String,
    pub revision: u32,
    /// None indicates a deleted ID whose revision is retained.
    pub preset: Option<Preset>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PresetSummary {
    pub id: String,
    pub revision: u32,
    pub name: String,
    pub kind: PresetKind,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PresetKind {
    Voice,
    Character,
    Template,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn instantiation_requires_current_atomic_track_protocol() {
        let args = InstantiatePresetArgs {
            session_id: "session".into(),
            operation_id: "operation".into(),
            expected_revision: 0,
            preset_id: "preset".into(),
            preset_revision: 1,
            ids: sungeki_editor::CharacterPresetIds {
                character_id: "character".into(),
                asset_ids: Default::default(),
            },
            placement: None,
        };
        assert_eq!(
            crate::SessionRequest::InstantiatePreset { args }.minimum_protocol_minor(),
            129
        );
    }
    #[test]
    fn requests_require_preset_protocol() {
        assert_eq!(
            crate::SessionRequest::Presets {
                request: PresetRequest::List {
                    after_id: None,
                    limit: 20
                },
            }
            .minimum_protocol_minor(),
            127
        );
    }
}
