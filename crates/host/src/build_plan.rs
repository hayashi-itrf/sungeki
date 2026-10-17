//! Fixed identities allocated once before parent persistence, never on replay.
use super::invalid;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use sungeki_editor::{BuildForewordIds, BuildLayoutIds, BuildSceneVisualIds, BuildScriptIds};
use sungeki_project::BuildRecipe;
use sungeki_protocol::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Plan {
    pub project_id: String,
    pub script: BuildScriptIds,
    pub layout: BuildLayoutIds,
    pub speech_asset_ids: Vec<String>,
    pub speech_operation_ids: Vec<String>,
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
impl Plan {
    pub fn new(recipe: &BuildRecipe) -> Self {
        let count: usize = recipe.scenes.iter().map(|s| s.lines.len()).sum();
        Self {
            project_id: id(),
            script: BuildScriptIds {
                track_id: id(),
                scene_ids: recipe.scenes.iter().map(|_| id()).collect(),
                speech_ids: (0..count).map(|_| id()).collect(),
            },
            layout: BuildLayoutIds {
                tracks: std::array::from_fn(|_| id()),
                scenes: recipe
                    .scenes
                    .iter()
                    .map(|s| BuildSceneVisualIds {
                        background: id(),
                        standing: s.lines.iter().map(|_| [id(), id()]).collect(),
                        diagrams: s.diagrams.iter().map(|_| id()).collect(),
                    })
                    .collect(),
                title_scene: id(),
                title_clips: [id(), id()],
                credits_scene: id(),
                credits_clips: [id(), id()],
                bgm: recipe.bgm.as_ref().map(|_| id()),
                foreword: recipe.foreword.as_ref().map(|_| BuildForewordIds {
                    scene: id(),
                    track: id(),
                    clips: std::array::from_fn(|_| id()),
                }),
            },
            speech_asset_ids: (0..count).map(|_| id()).collect(),
            speech_operation_ids: (0..count).map(|_| id()).collect(),
        }
    }
    pub fn validate(&self, recipe: &BuildRecipe, steps: &[String]) -> Result<(), ApiError> {
        let count: usize = recipe.scenes.iter().map(|s| s.lines.len()).sum();
        if self.script.scene_ids.len() != recipe.scenes.len()
            || self.script.speech_ids.len() != count
            || self.speech_asset_ids.len() != count
            || self.speech_operation_ids.len() != count
            || self.layout.scenes.len() != recipe.scenes.len()
            || self.layout.bgm.is_some() != recipe.bgm.is_some()
            || self.layout.foreword.is_some() != recipe.foreword.is_some()
            || self
                .layout
                .scenes
                .iter()
                .zip(&recipe.scenes)
                .any(|(ids, s)| {
                    ids.standing.len() != s.lines.len() || ids.diagrams.len() != s.diagrams.len()
                })
        {
            return Err(invalid("固定IDの件数が制作入力と一致しません"));
        }
        // Plan contains only identities, arrays/objects of identities, and null.
        let mut values = Vec::new();
        collect(&serde_json::to_value(self).map_err(invalid)?, &mut values)?;
        values.extend(steps.iter().cloned());
        values.extend(recipe.assets.iter().map(|a| a.id.clone()));
        values.extend(recipe.roles.iter().map(|r| r.character.id.clone()));
        let mut seen = HashSet::new();
        for value in values {
            let parsed = uuid::Uuid::parse_str(&value).map_err(invalid)?;
            if parsed.hyphenated().to_string() != value || !seen.insert(value) {
                return Err(invalid("固定IDは正規形のUUIDで重複なく指定してください"));
            }
        }
        Ok(())
    }
}
fn collect(value: &serde_json::Value, ids: &mut Vec<String>) -> Result<(), ApiError> {
    match value {
        serde_json::Value::String(id) => ids.push(id.clone()),
        serde_json::Value::Array(values) => {
            for value in values {
                collect(value, ids)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                collect(value, ids)?;
            }
        }
        serde_json::Value::Null => {}
        _ => return Err(invalid("固定計画にはIDだけを保存してください")),
    }
    Ok(())
}
