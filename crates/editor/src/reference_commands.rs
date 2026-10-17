//! Resolve typed editing targets. Never recursively substitute arbitrary strings.
use crate::{
    Command,
    batch_references::{optional, resolve as id},
};
use std::collections::BTreeMap;
use sungeki_project::ValidationError;

pub(crate) fn resolve(
    c: &mut Command,
    refs: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    match c {
        Command::SetText { clip_id, .. }
        | Command::SetTextOutline { clip_id, .. }
        | Command::SetTextLayout { clip_id, .. }
        | Command::SetTextStyle { clip_id, .. }
        | Command::SetBlur { clip_id, .. }
        | Command::SetShape { clip_id, .. }
        | Command::SetTransform { clip_id, .. }
        | Command::MoveClip { clip_id, .. }
        | Command::ResizeClip { clip_id, .. }
        | Command::DeleteClip { clip_id }
        | Command::SetSpeechGain { clip_id, .. }
        | Command::SetReactionMotion { clip_id, .. } => id(clip_id, refs)?,
        Command::SetAudio {
            clip_id, asset_id, ..
        }
        | Command::SetVideo {
            clip_id, asset_id, ..
        }
        | Command::SetImageAsset { clip_id, asset_id }
        | Command::SetPsd {
            clip_id, asset_id, ..
        } => {
            id(clip_id, refs)?;
            id(asset_id, refs)?;
        }
        Command::SetSpeech {
            clip_id,
            character_id,
            ..
        } => {
            id(clip_id, refs)?;
            id(character_id, refs)?;
        }
        Command::SetSpeechAudio { clip_id, audio } => {
            id(clip_id, refs)?;
            if let Some(audio) = audio {
                id(&mut audio.asset_id, refs)?;
            }
        }
        Command::SetPsdSpeech { clip_id, speech } => {
            id(clip_id, refs)?;
            if let Some(speech) = speech {
                id(&mut speech.character_id, refs)?;
            }
        }
        Command::SetSprite {
            clip_id,
            sprites,
            character_id,
            ..
        } => {
            id(clip_id, refs)?;
            optional(character_id, refs)?;
            for pose in &mut sprites.poses {
                id(&mut pose.closed_asset_id, refs)?;
                optional(&mut pose.open_asset_id, refs)?;
            }
        }
        Command::SetVrm { clip_id, standing } => {
            id(clip_id, refs)?;
            id(&mut standing.model_asset_id, refs)?;
            id(&mut standing.motion_asset_id, refs)?;
            optional(&mut standing.character_id, refs)?;
        }
        Command::SetClipTrack { clip_id, track_id } => {
            id(clip_id, refs)?;
            id(track_id, refs)?;
        }
        Command::SetClipScene { clip_id, scene_id } => {
            id(clip_id, refs)?;
            optional(scene_id, refs)?;
        }
        Command::SetTrackProperties { track_id, .. } => id(track_id, refs)?,
        Command::ReorderTracks { track_ids } => list(track_ids, refs)?,
        Command::LayoutSpeech { clip_ids, .. } | Command::Ripple { clip_ids, .. } => {
            list(clip_ids, refs)?
        }
        Command::LayoutScenes { scene_ids, .. } => list(scene_ids, refs)?,
        Command::SetSceneFade { scene_id, .. }
        | Command::RenameScene { scene_id, .. }
        | Command::ResizeScene { scene_id, .. }
        | Command::DeleteScene { scene_id }
        | Command::UngroupScene { scene_id }
        | Command::MoveScene { scene_id, .. } => id(scene_id, refs)?,
        Command::RenameGroup { group_id, .. }
        | Command::MoveGroup { group_id, .. }
        | Command::Ungroup { group_id } => id(group_id, refs)?,
        Command::RemoveCharacter { character_id }
        | Command::ApplyVoicePreset { character_id, .. } => id(character_id, refs)?,
        Command::SetAssetMetadata { asset_id, .. }
        | Command::RelinkAsset { asset_id, .. }
        | Command::RemoveAsset { asset_id } => id(asset_id, refs)?,
        Command::RefreshSceneCredits { clip_id, scene_id } => {
            id(clip_id, refs)?;
            id(scene_id, refs)?;
        }
        Command::FitVisualToSpeech {
            clip_id,
            first_speech_id,
            last_speech_id,
        } => {
            id(clip_id, refs)?;
            id(first_speech_id, refs)?;
            id(last_speech_id, refs)?;
        }
        Command::MoveSelection {
            scene_ids,
            group_ids,
            clip_ids,
            ..
        } => {
            list(scene_ids, refs)?;
            list(group_ids, refs)?;
            list(clip_ids, refs)?;
        }
        Command::DuplicateClip { clip_id, .. } => id(clip_id, refs)?,
        Command::DuplicateScene {
            scene_id,
            clip_ids,
            group_ids,
            ..
        } => {
            id(scene_id, refs)?;
            source_map(clip_ids, refs)?;
            source_map(group_ids, refs)?;
        }
        Command::DuplicateGroup {
            group_id, clip_ids, ..
        } => {
            id(group_id, refs)?;
            source_map(clip_ids, refs)?;
        }
        Command::AppendSpeechScript { script } => {
            id(&mut script.character_id, refs)?;
            id(&mut script.track_id, refs)?;
            optional(&mut script.scene_id, refs)?;
            for line in &mut script.lines {
                optional(&mut line.character_id, refs)?;
            }
        }
        Command::PrependForeword { template } => id(&mut template.background_asset_id, refs)?,
        Command::ApplyDialogueTemplate { template } => {
            list(&mut template.speech_clip_ids, refs)?;
            id(&mut template.background_clip_id, refs)?;
            id(&mut template.bgm_clip_id, refs)?;
            for participant in &mut template.participants {
                id(&mut participant.character_id, refs)?;
                id(&mut participant.standing_clip_id, refs)?;
            }
        }
        Command::ApplyCharacterPreset {
            placement: Some(placement),
            ..
        } => {
            if placement.new_track_name.is_none() {
                id(&mut placement.track_id, refs)?;
            }
            optional(&mut placement.scene_id, refs)?;
        }
        // Generated destination IDs and preset-local ID maps keep explicit UUIDs.
        // Creation declarations and nested references are handled by the planner.
        _ => {}
    }
    Ok(())
}
fn list(ids: &mut [String], refs: &BTreeMap<String, String>) -> Result<(), ValidationError> {
    for value in ids {
        id(value, refs)?;
    }
    Ok(())
}

// Resolve source keys only. Destination IDs remain explicit fresh UUIDs.
// Two distinct spellings can point at the same source: reject rather than
// silently dropping one entry while collecting the map.
fn source_map(
    mapping: &mut BTreeMap<String, String>,
    refs: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    let mut resolved = BTreeMap::new();
    for (source, destination) in mapping.iter() {
        let mut source = source.clone();
        id(&mut source, refs)?;
        if resolved.insert(source, destination.clone()).is_some() {
            return Err(ValidationError::new("複製元の参照が重複しています".into()));
        }
    }
    *mapping = resolved;
    Ok(())
}
