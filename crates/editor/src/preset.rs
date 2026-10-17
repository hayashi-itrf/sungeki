//! Pure planning: library access and file checks belong to the host. Apply the
//! returned commands in one normal EditRequest, preserving existing Undo rules.
use crate::{Command, apply_batch};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use sungeki_project::*;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CharacterPresetIds {
    pub character_id: String,
    /// Complete mapping from preset-local asset IDs to newly allocated IDs.
    pub asset_ids: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PresetStandingPlacement {
    /// Explicit opt-in to create track_id in the same atomic edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub new_track_name: Option<String>,
    pub clip_id: String,
    pub track_id: String,
    pub scene_id: Option<String>,
    pub start_us: TimeUs,
    pub duration_us: TimeUs,
    pub transform: Transform,
}

pub fn plan_character_preset(
    project: &Project,
    preset: &Preset,
    ids: &CharacterPresetIds,
    placement: Option<&PresetStandingPlacement>,
) -> Result<Vec<Command>, ValidationError> {
    let commands = plan_character_preset_candidate(project, preset, ids, placement)?;
    apply_batch(project, &commands)?;
    Ok(commands)
}

/// Expand a nested command without treating its intermediate layout as committed.
/// The outer transaction owns the final track-collision check.
pub(crate) fn plan_character_preset_candidate(
    project: &Project,
    preset: &Preset,
    ids: &CharacterPresetIds,
    placement: Option<&PresetStandingPlacement>,
) -> Result<Vec<Command>, ValidationError> {
    preset.validate()?;
    let PresetContent::Character {
        character,
        assets,
        standing,
    } = &preset.content
    else {
        return Err(ValidationError::new(
            "キャラクタープリセットを指定してください".into(),
        ));
    };
    if standing.is_some() != placement.is_some() {
        return Err(ValidationError::new(
            "立ち絵の有無と配置指定を一致させてください".into(),
        ));
    }
    if ids.asset_ids.len() != assets.len()
        || assets.iter().any(|a| !ids.asset_ids.contains_key(&a.id))
    {
        return Err(ValidationError::new(
            "プリセット素材のID対応が不足または過剰です".into(),
        ));
    }
    let mut used: HashSet<&str> = std::iter::once(project.id.as_str())
        .chain(project.characters.iter().map(|v| v.id.as_str()))
        .chain(project.assets.iter().map(|v| v.id.as_str()))
        .chain(project.tracks.iter().map(|v| v.id.as_str()))
        .chain(project.scenes.iter().map(|v| v.id.as_str()))
        .chain(project.groups.iter().map(|v| v.id.as_str()))
        .chain(project.clips.iter().map(|v| v.id.as_str()))
        .collect();
    for id in std::iter::once(&ids.character_id)
        .chain(ids.asset_ids.values())
        .chain(placement.map(|p| &p.clip_id))
    {
        if !used.insert(id.as_str()) {
            return Err(ValidationError::new(
                "プリセット適用先の新規IDが重複しています".into(),
            ));
        }
    }
    let mut commands = Vec::new();
    if let Some(placement) = placement
        && let Some(name) = &placement.new_track_name
    {
        if !used.insert(placement.track_id.as_str())
            || name.trim().is_empty()
            || name.len() > 1024
            || name.contains('\0')
        {
            return Err(ValidationError::new(
                "新規トラックのID・名前が不正です".into(),
            ));
        }
        commands.push(Command::AddTrack {
            track: Track {
                id: placement.track_id.clone(),
                name: name.clone(),
                kind: TrackKind::Visual,
                visible: true,
                muted: false,
            },
        });
    }
    for asset in assets {
        let mut asset = asset.clone();
        asset.id = ids.asset_ids[&asset.id].clone();
        commands.push(Command::RegisterAsset { asset });
    }
    let mut character = character.clone();
    character.id = ids.character_id.clone();
    commands.push(Command::SetCharacter { character });
    if let (Some(standing), Some(placement)) = (standing, placement) {
        let mut transform = placement.transform.clone();
        let (content, flip) = rebind(standing, ids);
        transform.flip_x = Some(flip);
        let mut clip = Clip {
            id: placement.clip_id.clone(),
            track_id: placement.track_id.clone(),
            scene_id: placement.scene_id.clone(),
            start_us: placement.start_us,
            duration_us: placement.duration_us,
            transform,
            content,
        };
        let registered: Vec<_> = commands
            .iter()
            .filter_map(|command| match command {
                Command::RegisterAsset { asset } => Some(asset.clone()),
                _ => None,
            })
            .collect();
        crate::preset_geometry::fit(project, &registered, &mut clip)?;
        commands.push(Command::AddClip { clip });
    }
    crate::transaction::candidate(project, &commands)?;
    Ok(commands)
}

fn rebind(standing: &BuildStanding, ids: &CharacterPresetIds) -> (ClipContent, bool) {
    match standing {
        BuildStanding::Image { asset_id, flip_x } => (
            ClipContent::Image {
                asset_id: ids.asset_ids[asset_id].clone(),
            },
            *flip_x,
        ),
        BuildStanding::Psd {
            asset_id,
            pose,
            speech,
            flip_x,
        } => {
            let mut speech = speech.clone();
            speech.character_id = ids.character_id.clone();
            (
                ClipContent::Psd {
                    asset_id: ids.asset_ids[asset_id].clone(),
                    pose: pose.clone(),
                    speech: Some(speech),
                },
                *flip_x,
            )
        }
        BuildStanding::Sprite {
            sprites,
            pose,
            flip_x,
        } => {
            let mut sprites = sprites.clone();
            for pose in &mut sprites.poses {
                pose.closed_asset_id = ids.asset_ids[&pose.closed_asset_id].clone();
                pose.open_asset_id = pose
                    .open_asset_id
                    .as_ref()
                    .map(|id| ids.asset_ids[id].clone());
            }
            (
                ClipContent::Sprite {
                    sprites,
                    pose: pose.clone(),
                    character_id: Some(ids.character_id.clone()),
                },
                *flip_x,
            )
        }
        BuildStanding::Vrm { standing, flip_x } => {
            let mut standing = standing.clone();
            standing.model_asset_id = ids.asset_ids[&standing.model_asset_id].clone();
            standing.motion_asset_id = ids.asset_ids[&standing.motion_asset_id].clone();
            standing.character_id = Some(ids.character_id.clone());
            (ClipContent::Vrm { standing }, *flip_x)
        }
    }
}
