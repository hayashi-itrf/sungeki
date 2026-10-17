//! Typed creation planning. Only ID fields are resolved; text and paths are literals.
use crate::{Command, apply_batch};
use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::*;

pub struct ReferenceCreationPlan {
    pub commands: Vec<Command>,
    pub created_ids: BTreeMap<String, String>,
}

/// `$name` in a creation's own ID declares a batch-local reference. The caller
/// fixes its UUID allocation before planning. References must precede their use.
/// Commands are applied atomically by the normal EditSession after planning.
pub fn plan_batch_references(
    project: &Project,
    commands: &[Command],
    allocations: &BTreeMap<String, String>,
) -> Result<ReferenceCreationPlan, ValidationError> {
    let allocated_ids: BTreeSet<&str> = allocations.values().map(String::as_str).collect();
    let mut known = BTreeMap::new();
    let mut used: BTreeSet<String> = std::iter::once(&project.id)
        .chain(project.tracks.iter().map(|x| &x.id))
        .chain(project.characters.iter().map(|x| &x.id))
        .chain(project.assets.iter().map(|x| &x.id))
        .chain(project.scenes.iter().map(|x| &x.id))
        .chain(project.groups.iter().map(|x| &x.id))
        .chain(project.clips.iter().map(|x| &x.id))
        .cloned()
        .collect();
    let mut resolved = commands.to_vec();
    for command in &mut resolved {
        let update_character = matches!(command, Command::SetCharacter { character }
            if character.id.strip_prefix('$').is_some_and(|name| known.contains_key(name)));
        if update_character {
            if let Command::SetCharacter { character } = command {
                resolve(&mut character.id, &known)?;
            }
            continue;
        }
        let id = match command {
            Command::AddTrack { track } => Some(&mut track.id),
            Command::SetCharacter { character } => Some(&mut character.id),
            Command::RegisterAsset { asset } => Some(&mut asset.id),
            Command::AddScene { scene } => Some(&mut scene.id),
            Command::AddGroup { group } => Some(&mut group.id),
            Command::AddClip { clip } => Some(&mut clip.id),
            _ => None,
        };
        if let Some(id) = id {
            if let Some(name) = id.strip_prefix('$') {
                if name.is_empty()
                    || name.len() > 128
                    || name
                        .chars()
                        .any(|c| c.is_control() || c.is_whitespace() || c == '$')
                {
                    return Err(invalid("参照名が不正です"));
                }
                let allocated = allocations
                    .get(name)
                    .ok_or_else(|| invalid("参照のID割当がありません"))?;
                let uuid = uuid::Uuid::parse_str(allocated)
                    .map_err(|_| invalid("割当IDがUUIDではありません"))?;
                if uuid.hyphenated().to_string() != *allocated
                    || known.insert(name.to_owned(), allocated.clone()).is_some()
                {
                    return Err(invalid("参照名または割当IDが不正です"));
                }
                *id = allocated.clone();
            }
            // Allocated IDs may never silently replace an existing person.
            if allocated_ids.contains(id.as_str()) && !used.insert(id.clone()) {
                return Err(invalid("作成IDが既存または別の作成IDと重複しています"));
            }
        }
        match command {
            Command::AddClip { clip } => resolve_clip(clip, &known)?,
            Command::AddGroup { group } => {
                for id in &mut group.clip_ids {
                    resolve(id, &known)?;
                }
            }
            _ => crate::reference_commands::resolve(command, &known)?,
        }
    }
    if known.len() != allocations.len() {
        return Err(invalid("未使用の参照ID割当があります"));
    }
    apply_batch(project, &resolved)?;
    Ok(ReferenceCreationPlan {
        commands: resolved,
        created_ids: known,
    })
}
fn invalid(message: &str) -> ValidationError {
    ValidationError::new(message.into())
}
pub(crate) fn resolve(
    id: &mut String,
    known: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    if let Some(name) = id.strip_prefix('$') {
        *id = known
            .get(name)
            .ok_or_else(|| invalid("まだ宣言されていない参照は使用できません"))?
            .clone();
    }
    Ok(())
}
pub(crate) fn optional(
    id: &mut Option<String>,
    known: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    if let Some(id) = id {
        resolve(id, known)?;
    }
    Ok(())
}
fn resolve_clip(clip: &mut Clip, known: &BTreeMap<String, String>) -> Result<(), ValidationError> {
    resolve(&mut clip.track_id, known)?;
    optional(&mut clip.scene_id, known)?;
    match &mut clip.content {
        ClipContent::Image { asset_id }
        | ClipContent::Video { asset_id, .. }
        | ClipContent::Audio { asset_id, .. } => resolve(asset_id, known)?,
        ClipContent::Psd {
            asset_id, speech, ..
        } => {
            resolve(asset_id, known)?;
            if let Some(speech) = speech {
                resolve(&mut speech.character_id, known)?;
            }
        }
        ClipContent::Speech {
            character_id,
            audio,
            ..
        } => {
            resolve(character_id, known)?;
            if let Some(audio) = audio {
                resolve(&mut audio.asset_id, known)?;
            }
        }
        ClipContent::Sprite {
            sprites,
            character_id,
            ..
        } => {
            optional(character_id, known)?;
            for pose in &mut sprites.poses {
                resolve(&mut pose.closed_asset_id, known)?;
                optional(&mut pose.open_asset_id, known)?;
            }
        }
        ClipContent::Vrm { standing } => {
            resolve(&mut standing.model_asset_id, known)?;
            resolve(&mut standing.motion_asset_id, known)?;
            optional(&mut standing.character_id, known)?;
        }
        ClipContent::Text { .. } | ClipContent::Shape { .. } | ClipContent::Blur { .. } => {}
    }
    Ok(())
}
