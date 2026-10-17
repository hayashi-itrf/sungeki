//! Apply one verified synthesis result to a build staging document. Host must
//! verify the captured input, original bytes and revision before committing.
use crate::{BuildScriptIds, Command, apply_batch, plan_build_script};
use sungeki_project::*;

fn invalid() -> ValidationError {
    ValidationError::new("制作台本が変更済みか、合成結果の順序・参照が不正です".into())
}

pub fn plan_build_speech_application(
    project: &Project,
    recipe: &BuildRecipe,
    ids: &BuildScriptIds,
    clip_id: &str,
    asset: Asset,
    audio: SpeechAudio,
) -> Result<Vec<Command>, ValidationError> {
    let (mut durations, next) = validate_stage(project, recipe, ids)?;
    if ids.speech_ids.get(next).map(String::as_str) != Some(clip_id) || audio.asset_id != asset.id {
        return Err(invalid());
    }
    let AssetProbe::Audio(probe) = &asset.probe else {
        return Err(invalid());
    };
    durations[next] = probe.duration_us;
    let mut commands = timing(project, recipe, ids, &durations)?;
    commands.push(Command::RegisterAsset { asset });
    commands.push(Command::SetSpeechAudio {
        clip_id: clip_id.into(),
        audio: Some(audio),
    });
    apply_batch(project, &commands)?;
    Ok(commands)
}

pub(crate) fn validate_stage(
    project: &Project,
    recipe: &BuildRecipe,
    ids: &BuildScriptIds,
) -> Result<(Vec<TimeUs>, usize), ValidationError> {
    validate(project)?;
    let empty = Project::empty(project.id.clone(), recipe.title.clone());
    let base = apply_batch(&empty, &plan_build_script(&empty, recipe, ids)?)?;
    if project.clips.len() != base.clips.len() {
        return Err(invalid());
    }
    let mut durations = vec![TimeUs(1_000_000); ids.speech_ids.len()];
    let mut attached = Vec::new();
    let mut next = 0;
    let mut missing = false;
    for (index, id) in ids.speech_ids.iter().enumerate() {
        let clip = project
            .clips
            .iter()
            .find(|c| c.id == *id)
            .ok_or_else(invalid)?;
        let ClipContent::Speech { audio, .. } = &clip.content else {
            return Err(invalid());
        };
        if let Some(audio) = audio {
            if missing {
                return Err(invalid());
            }
            let asset = project
                .assets
                .iter()
                .find(|a| a.id == audio.asset_id)
                .ok_or_else(invalid)?;
            let AssetProbe::Audio(probe) = &asset.probe else {
                return Err(invalid());
            };
            durations[index] = probe.duration_us;
            attached.push(Command::RegisterAsset {
                asset: asset.clone(),
            });
            attached.push(Command::SetSpeechAudio {
                clip_id: id.clone(),
                audio: Some(audio.clone()),
            });
            next += 1;
        } else {
            missing = true;
        }
    }
    // Reconstruct the only allowed staging state through shared commands. This
    // catches edits to text, membership, timing, settings and unrelated assets.
    let mut expected_commands = timing(&base, recipe, ids, &durations)?;
    expected_commands.extend(attached);
    if apply_batch(&base, &expected_commands)? != *project {
        return Err(invalid());
    }
    Ok((durations, next))
}

fn timing(
    project: &Project,
    recipe: &BuildRecipe,
    ids: &BuildScriptIds,
    durations: &[TimeUs],
) -> Result<Vec<Command>, ValidationError> {
    let mut commands = Vec::new();
    let mut placements = Vec::new();
    let mut cursor = 0_u64;
    let mut offset = 0;
    for (index, scene) in recipe.scenes.iter().enumerate() {
        let start = cursor;
        for line_index in 0..scene.lines.len() {
            let duration = durations[offset];
            placements.push((ids.speech_ids[offset].clone(), cursor, duration));
            cursor = cursor
                .checked_add(duration.0)
                .filter(|end| duration.0 > 0 && *end <= MAX_TIME_US)
                .ok_or_else(invalid)?;
            if line_index + 1 < scene.lines.len() {
                cursor = cursor
                    .checked_add(scene.gap_us.0)
                    .filter(|end| *end <= MAX_TIME_US)
                    .ok_or_else(invalid)?;
            }
            offset += 1;
        }
        commands.push(Command::MoveScene {
            scene_id: ids.scene_ids[index].clone(),
            start_us: TimeUs(start),
        });
        commands.push(Command::ResizeScene {
            scene_id: ids.scene_ids[index].clone(),
            duration_us: TimeUs(cursor - start),
        });
    }
    // MoveScene moves its members too, so set absolute speech positions after
    // all scenes have moved. One final batch validation avoids partial states.
    for (clip_id, start, duration) in placements {
        if !project.clips.iter().any(|c| c.id == clip_id) {
            return Err(invalid());
        }
        commands.push(Command::MoveClip {
            clip_id: clip_id.clone(),
            start_us: TimeUs(start),
        });
        commands.push(Command::ResizeClip {
            clip_id,
            duration_us: duration,
        });
    }
    Ok(commands)
}
