//! Sungeki-only reusable settings; file contents are never embedded or copied.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub schema_version: u32,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub content: PresetContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresetContent {
    Voice {
        voice: Voice,
        credit: String,
    },
    Character {
        character: Character,
        assets: Vec<Asset>,
        standing: Option<BuildStanding>,
    },
    Template {
        recipe: BuildRecipe,
    },
}

fn text(value: &str, max: usize, empty: bool) -> bool {
    value.len() <= max && !value.contains('\0') && (empty || !value.trim().is_empty())
}

impl Preset {
    /// Structural checks only. Host resolves source paths and verifies actual
    /// files before application. Existing Projects are never linked dynamically.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != 1
            || !text(&self.name, 1024, false)
            || !text(&self.description, 65536, true)
            || self.tags.len() > 32
            || self.tags.iter().any(|tag| !text(tag, 128, false))
            || self
                .tags
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.tags.len()
        {
            return Err(ValidationError::new(
                "プリセットの版・名前・説明・タグが不正です".into(),
            ));
        }
        match &self.content {
            PresetContent::Voice { voice, credit } => {
                let mut project =
                    Project::empty(uuid::Uuid::from_u128(0).to_string(), self.name.clone());
                project.characters.push(Character {
                    id: uuid::Uuid::from_u128(1).to_string(),
                    name: self.name.clone(),
                    credit: credit.clone(),
                    voice: voice.clone(),
                    subtitle_style: SubtitleStyle {
                        layout: None,
                        outline: None,
                        font_size: 48.,
                        color: "#ffffff".into(),
                    },
                });
                require_credit(credit)?;
                validate(&project)
            }
            PresetContent::Character {
                character,
                assets,
                standing,
            } => {
                validate_locations(assets)?;
                require_credit(&character.credit)?;
                let id = (0..)
                    .map(|n| uuid::Uuid::from_u128(n).to_string())
                    .find(|id| id != &character.id && !assets.iter().any(|a| &a.id == id))
                    .unwrap();
                let mut project = Project::empty(id, self.name.clone());
                project.characters.push(character.clone());
                project.assets = assets.clone();
                validate(&project)?;
                if let Some(standing) = standing {
                    standing.validate_character(&project, &character.id)?;
                }
                Ok(())
            }
            PresetContent::Template { recipe } => {
                validate_locations(&recipe.assets)?;
                recipe.validate()
            }
        }
    }
}

fn require_credit(credit: &str) -> Result<(), ValidationError> {
    if !text(credit, 65536, false) {
        return Err(ValidationError::new(
            "プリセットにはクレジットが必要です".into(),
        ));
    }
    Ok(())
}

fn validate_locations(assets: &[Asset]) -> Result<(), ValidationError> {
    if assets.len() > 512 {
        return Err(ValidationError::new(
            "プリセットの素材は512個以内です".into(),
        ));
    }
    for asset in assets {
        if matches!(asset.locator, AssetLocator::ProjectRelative { .. }) {
            return Err(ValidationError::new("プリセットの素材は作品相対パスでは保存できません。保存元で絶対パスへ解決してください".into()));
        }
        require_credit(&asset.credit)?;
    }
    Ok(())
}
