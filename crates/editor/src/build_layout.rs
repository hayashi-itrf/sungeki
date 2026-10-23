#[path = "build_bgm.rs"]
mod bgm;
#[path = "build_diagrams.rs"]
mod diagrams;
#[path = "build_opening.rs"]
mod opening;
#[path = "build_presentation.rs"]
mod presentation;
#[path = "build_structure.rs"]
mod structure;
use crate::{
    BuildScriptIds, Command, ForewordTemplate, apply_batch, build_visuals as visual,
    dialogue_template::rect,
};
use bgm::bgm_playback;
pub use opening::BuildOpeningIds;
use serde::{Deserialize, Serialize};
use sungeki_project::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSceneVisualIds {
    pub background: String,
    /// One pair per line. Portrait uses the second slot for the subtitle backdrop;
    /// wide uses left/right standing. Pose persists until that role changes it.
    pub standing: Vec<[String; 2]>,
    pub diagrams: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildForewordIds {
    pub scene: String,
    pub track: String,
    pub clips: [String; 4],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildLayoutIds {
    #[serde(default)]
    pub opening: Option<BuildOpeningIds>,
    /// Background, left, right, diagrams, music, cards.
    pub tracks: [String; 6],
    pub scenes: Vec<BuildSceneVisualIds>,
    pub title_scene: String,
    pub title_clips: [String; 2],
    pub credits_scene: String,
    pub credits_clips: [String; 2],
    pub bgm: Option<String>,
    pub foreword: Option<BuildForewordIds>,
}
fn error() -> ValidationError {
    ValidationError::new("映像構成の固定ID・合成状態が不正です".into())
}

/// Final layout of the fully synthesized, untouched staging document. IDs must
/// be reserved by the parent before executing this as one editor operation.
pub fn plan_build_layout(
    p: &Project,
    recipe: &BuildRecipe,
    speech: &BuildScriptIds,
    ids: &BuildLayoutIds,
) -> Result<Vec<Command>, ValidationError> {
    let (_, count) = crate::build_speech::validate_stage(p, recipe, speech)?;
    let opening = recipe
        .presentation
        .as_ref()
        .and_then(|p| p.opening.as_ref());
    let plain_title = recipe
        .presentation
        .as_ref()
        .and_then(|p| p.structure.as_ref())
        .and_then(|s| s.title_duration_us)
        .map_or(3_000_000, |t| t.0);
    let lead_us = opening.map_or(plain_title, |o| o.duration_us.0);
    if count != speech.speech_ids.len()
        || ids.scenes.len() != recipe.scenes.len()
        || ids.bgm.is_some() != recipe.bgm.is_some()
        || ids.foreword.is_some() != recipe.foreword.is_some()
        || ids.opening.is_some() != opening.is_some()
    {
        return Err(error());
    }
    let mut commands = Vec::new();
    for (i, name) in [
        "背景",
        "左の立ち絵",
        "右の立ち絵",
        "説明図",
        "BGM",
        "タイトル・クレジット",
    ]
    .iter()
    .enumerate()
    {
        commands.push(Command::AddTrack {
            track: Track {
                id: ids.tracks[i].clone(),
                name: if recipe.preset_id == "narration-portrait-v1" && i == 2 {
                    "字幕の背景".into()
                } else {
                    (*name).into()
                },
                visible: true,
                muted: false,
                kind: if i == 4 {
                    TrackKind::Audio
                } else {
                    TrackKind::Visual
                },
            },
        });
    }
    let behind = recipe
        .presentation
        .as_ref()
        .and_then(|p| p.slots.as_ref())
        .is_some_and(|s| s.diagrams_behind_standings);
    commands.push(Command::ReorderTracks {
        track_ids: vec![
            ids.tracks[0].clone(),
            ids.tracks[if behind { 3 } else { 1 }].clone(),
            ids.tracks[if behind { 1 } else { 2 }].clone(),
            ids.tracks[if behind { 2 } else { 3 }].clone(),
            speech.track_id.clone(),
            ids.tracks[4].clone(),
            ids.tracks[5].clone(),
        ],
    });
    let mut offset = 0;
    for (scene_index, (scene, visual_ids)) in recipe.scenes.iter().zip(&ids.scenes).enumerate() {
        if visual_ids.standing.len() != scene.lines.len()
            || visual_ids.diagrams.len() != scene.diagrams.len()
        {
            return Err(error());
        }
        let source = &p.scenes[scene_index];
        if let Some(fade) = &scene.fade {
            fade.validate(source.duration_us).map_err(|_| {
                ValidationError::new(format!(
                    "場面「{}」のフェード合計は合成後の場面尺以内で指定してください",
                    scene.name
                ))
            })?;
            commands.push(Command::SetSceneFade {
                scene_id: source.id.clone(),
                fade: Some(fade.clone()),
            });
        }
        let start = TimeUs(source.start_us.0 + lead_us);
        commands.push(Command::MoveScene {
            scene_id: source.id.clone(),
            start_us: start,
        });
        commands.push(Command::AddClip {
            clip: visual::visual(
                p,
                &visual_ids.background,
                &ids.tracks[0],
                &source.id,
                start,
                source.duration_us,
                ClipContent::Image {
                    asset_id: scene.background_asset_id.clone(),
                },
                0,
                false,
            )?,
        });
        let mut poses = [None, None];
        let mut mouth_bindings = [None, None];
        let mut sprite_poses = [None, None];
        for (line_index, line) in scene.lines.iter().enumerate() {
            let c = &p.clips[offset + line_index];
            commands.push(Command::SetTransform {
                clip_id: c.id.clone(),
                transform: if p.canvas.width == 1080 {
                    rect(80., 1480., 920., 320.)
                } else {
                    rect(360., 800., 1200., 240.)
                },
            });
            let role_index = recipe
                .roles
                .iter()
                .position(|r| r.role == line.role)
                .ok_or_else(error)?;
            if let Some(name) = &line.sprite_pose {
                sprite_poses[role_index] = Some(name.as_str());
            }
            if let Some(pose) = &line.pose {
                poses[role_index] = Some(pose);
                mouth_bindings[role_index] = None;
            }
            for (name, state) in line.psd_states.iter().flatten() {
                let index = recipe
                    .roles
                    .iter()
                    .position(|r| &r.role == name)
                    .ok_or_else(error)?;
                poses[index] = Some(&state.pose);
                mouth_bindings[index] = Some(&state.speech);
            }
            let end = if line_index + 1 < scene.lines.len() {
                p.clips[offset + line_index + 1].start_us.0
            } else {
                source.start_us.0 + source.duration_us.0
            };
            if recipe.preset_id == "narration-portrait-v1" {
                commands.push(Command::AddClip {
                    clip: Clip {
                        id: visual_ids.standing[line_index][1].clone(),
                        track_id: ids.tracks[2].clone(),
                        scene_id: Some(source.id.clone()),
                        start_us: TimeUs(c.start_us.0 + lead_us),
                        duration_us: TimeUs(end - c.start_us.0),
                        content: ClipContent::Shape {
                            shape: ShapeKind::Rectangle,
                            fill: "#000000".into(),
                        },
                        transform: Transform {
                            opacity: 0.82,
                            ..rect(50., 1450., 980., 380.)
                        },
                    },
                });
            }
            for (role_index, role) in recipe.roles.iter().enumerate() {
                let (mut content, flip) =
                    visual::standing(role, poses[role_index], sprite_poses[role_index]);
                if let (ClipContent::Psd { speech, .. }, Some(binding)) =
                    (&mut content, mouth_bindings[role_index])
                {
                    *speech = Some(binding.clone());
                }
                if let ClipContent::Vrm { standing } = &mut content {
                    standing.source_start_us.0 = standing
                        .source_start_us
                        .0
                        .checked_add(c.start_us.0 - source.start_us.0)
                        .ok_or_else(error)?;
                }
                commands.push(Command::AddClip {
                    clip: visual::visual(
                        p,
                        &visual_ids.standing[line_index][role_index],
                        &ids.tracks[role_index + 1],
                        &source.id,
                        TimeUs(c.start_us.0 + lead_us),
                        TimeUs(end - c.start_us.0),
                        content,
                        role_index + 1,
                        flip,
                    )?,
                });
            }
        }
        commands.extend(diagrams::plan(
            p,
            scene,
            source,
            &visual_ids.diagrams,
            offset,
            &ids.tracks[3],
            lead_us,
        )?);
        offset += scene.lines.len();
    }
    let speech_end = p
        .duration()
        .ok_or_else(error)?
        .0
        .checked_add(lead_us)
        .ok_or_else(error)?;
    let ending = recipe.presentation.as_ref().and_then(|p| p.ending.as_ref());
    let ending_duration = ending.map_or(3_500_000, |e| e.duration_us.0);
    let ending_background = ending.and_then(|e| e.background_asset_id.as_deref());
    let end = speech_end
        .checked_add(ending_duration)
        .filter(|end| *end <= MAX_TIME_US)
        .ok_or_else(error)?;
    if let (Some(bgm), Some(id)) = (&recipe.bgm, &ids.bgm) {
        commands.push(Command::AddClip {
            clip: Clip {
                id: id.clone(),
                track_id: ids.tracks[4].clone(),
                scene_id: None,
                start_us: TimeUs(0),
                duration_us: TimeUs(end),
                transform: rect(0., 0., p.canvas.width as f64, p.canvas.height as f64),
                content: ClipContent::Audio {
                    asset_id: bgm.asset_id.clone(),
                    playback: bgm_playback(bgm, TimeUs(end))?,
                },
            },
        });
    }
    if let (Some(opening), Some(extra)) = (opening, &ids.opening) {
        commands.extend(opening::plan(p, recipe, opening, ids, extra)?);
    } else {
        commands.extend(visual::card(
            p,
            &ids.title_scene,
            &ids.title_clips,
            &ids.tracks[5],
            "タイトル",
            recipe.title.clone(),
            0,
            lead_us,
            false,
            None,
        ));
    }
    crate::track_separation::complete_layout(p, &mut commands)?;
    let candidate = apply_batch(p, &commands)?;
    let selected = candidate
        .clips
        .iter()
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let mut credits = crate::credits::text_for_clips(&candidate, &selected)?;
    if let Some(id) = ending_background {
        let asset = recipe
            .assets
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(error)?;
        for line in asset.credit.lines() {
            if !credits.lines().any(|existing| existing == line) {
                credits.push('\n');
                credits.push_str(line);
            }
        }
    }
    commands.extend(visual::card(
        p,
        &ids.credits_scene,
        &ids.credits_clips,
        &ids.tracks[5],
        "クレジット",
        credits,
        speech_end,
        ending_duration,
        true,
        ending_background,
    ));
    if let (Some(foreword), Some(ids)) = (&recipe.foreword, &ids.foreword) {
        commands.push(Command::PrependForeword {
            template: ForewordTemplate {
                preset_id: foreword.preset_id.clone(),
                background_asset_id: recipe.scenes[0].background_asset_id.clone(),
                text: foreword.text.clone(),
                duration_us: foreword.duration_us,
                scene_id: ids.scene.clone(),
                track_id: ids.track.clone(),
                clip_ids: ids.clips.to_vec(),
            },
        });
    }
    crate::track_separation::complete_layout(p, &mut commands)?;
    let candidate = apply_batch(p, &commands)?;
    commands.extend(structure::plan(&candidate, recipe, speech, ids)?);
    let candidate = apply_batch(p, &commands)?;
    commands.extend(presentation::plan(&candidate, recipe, ids)?);
    apply_batch(p, &commands)?;
    Ok(commands)
}
