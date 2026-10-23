//! Resolve authored section order in the initial common-editor transaction.
use crate::{BuildLayoutIds, BuildScriptIds, Command};
use sungeki_project::*;

pub(super) fn plan(
    p: &Project,
    recipe: &BuildRecipe,
    speech: &BuildScriptIds,
    ids: &BuildLayoutIds,
) -> Result<Vec<Command>, ValidationError> {
    let Some(structure) = recipe
        .presentation
        .as_ref()
        .and_then(|p| p.structure.as_ref())
    else {
        return Ok(vec![]);
    };
    structure.validate(recipe)?;
    let block_scenes = |b: &BuildBlock| -> Vec<String> {
        match b {
            BuildBlock::Foreword => ids
                .foreword
                .as_ref()
                .map(|f| vec![f.scene.clone()])
                .unwrap_or_default(),
            BuildBlock::Opening => vec![ids.title_scene.clone()],
            BuildBlock::Body => speech.scene_ids.clone(),
            BuildBlock::Credits => vec![ids.credits_scene.clone()],
        }
    };
    let mut commands = vec![];
    for block in [BuildBlock::Foreword, BuildBlock::Opening] {
        if !structure.blocks.contains(&block) {
            for scene_id in block_scenes(&block) {
                commands.push(Command::DeleteScene { scene_id });
            }
        }
    }
    let scene_ids: Vec<_> = structure.blocks.iter().flat_map(block_scenes).collect();
    let duration = scene_ids.iter().try_fold(0u64, |sum, id| {
        p.scenes
            .iter()
            .find(|s| &s.id == id)
            .and_then(|s| sum.checked_add(s.duration_us.0))
            .filter(|t| *t <= MAX_TIME_US)
            .ok_or_else(|| ValidationError::new("構成の尺・シーン参照が不正です".into()))
    })?;
    commands.push(Command::LayoutScenes {
        scene_ids,
        start_us: TimeUs(0),
        gap_us: TimeUs(0),
    });
    if let (Some(bgm), Some(id)) = (&recipe.bgm, &ids.bgm) {
        let mut playback = super::bgm::bgm_playback(bgm, TimeUs(duration))?;
        playback.fade_in_us.0 = playback.fade_in_us.0.min(duration);
        playback.fade_out_us.0 = playback.fade_out_us.0.min(duration - playback.fade_in_us.0);
        commands.extend([
            Command::MoveClip {
                clip_id: id.clone(),
                start_us: TimeUs(0),
            },
            Command::ResizeClip {
                clip_id: id.clone(),
                duration_us: TimeUs(duration),
            },
            Command::SetAudio {
                clip_id: id.clone(),
                asset_id: bgm.asset_id.clone(),
                playback,
            },
        ]);
    }
    Ok(commands)
}
