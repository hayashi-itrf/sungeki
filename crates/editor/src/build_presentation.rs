//! Part of the initial layout transaction, before save, review or rendering.
use crate::{BuildLayoutIds, Command};
use sungeki_project::*;

pub(super) fn plan(
    p: &Project,
    recipe: &BuildRecipe,
    ids: &BuildLayoutIds,
) -> Result<Vec<Command>, ValidationError> {
    let Some(settings) = &recipe.presentation else {
        return Ok(vec![]);
    };
    settings.validate(recipe)?;
    let mut commands = vec![];
    for scene in &ids.scenes {
        for pair in &scene.standing {
            for (role_index, id) in pair.iter().take(recipe.roles.len()).enumerate() {
                let clip = p.clips.iter().find(|c| &c.id == id).unwrap();
                let mut transform = settings
                    .slots
                    .as_ref()
                    .and_then(|s| s.standing.get(&recipe.roles[role_index].role))
                    .cloned()
                    .unwrap_or_else(|| clip.transform.clone());
                if settings
                    .slots
                    .as_ref()
                    .is_none_or(|s| !s.standing.contains_key(&recipe.roles[role_index].role))
                {
                    transform.y += settings.standing_offset_y;
                }
                transform = fit_visual_transform(
                    &p.assets,
                    &clip.content,
                    &transform,
                    settings
                        .fitting
                        .as_ref()
                        .and_then(|f| f.standing.get(&recipe.roles[role_index].role)),
                )?;
                commands.push(Command::SetTransform {
                    clip_id: id.clone(),
                    transform,
                });
            }
        }
    }
    if let Some(slots) = &settings.slots {
        for c in &p.clips {
            if matches!(c.content, ClipContent::Speech { .. })
                && let Some(transform) = &slots.subtitle
            {
                commands.push(Command::SetTransform {
                    clip_id: c.id.clone(),
                    transform: transform.clone(),
                });
            }
        }
        if let Some(transform) = &slots.diagram {
            for id in ids.scenes.iter().flat_map(|s| &s.diagrams) {
                let clip = p.clips.iter().find(|c| &c.id == id).unwrap();
                let mut transform = transform.clone();
                // A diagram's own fade remains relative to its synthesized range.
                transform.fade = clip.transform.fade.clone();
                transform = fit_visual_transform(
                    &p.assets,
                    &clip.content,
                    &transform,
                    settings.fitting.as_ref().and_then(|f| f.diagram.as_ref()),
                )?;
                commands.push(Command::SetTransform {
                    clip_id: id.clone(),
                    transform,
                });
            }
        }
    }
    if let Some(style) = &settings.credits {
        let clip_id = ids.credits_clips[1].clone();
        let clip = p.clips.iter().find(|c| c.id == clip_id).unwrap();
        let ClipContent::Text { text, .. } = &clip.content else {
            unreachable!()
        };
        let text = text
            .lines()
            .filter(|line| {
                !style
                    .omit_lines_containing
                    .iter()
                    .any(|omit| line.contains(omit))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            return Err(ValidationError::new(
                "クレジットを全行除外することはできません".into(),
            ));
        }
        if settings
            .ending
            .as_ref()
            .and_then(|e| e.background_asset_id.as_ref())
            .is_none()
        {
            commands.push(Command::SetShape {
                clip_id: ids.credits_clips[0].clone(),
                shape: ShapeKind::Rectangle,
                fill: style.background.clone(),
            });
        }
        commands.extend([
            Command::SetText {
                clip_id: clip_id.clone(),
                text,
            },
            Command::SetTextStyle {
                clip_id: clip_id.clone(),
                font_size: style.font_size,
                color: style.color.clone(),
            },
            Command::SetTextLayout {
                clip_id: clip_id.clone(),
                layout: Some(style.layout.clone()),
            },
            Command::SetTransform {
                clip_id,
                transform: style.transform.clone(),
            },
            Command::SetSceneFade {
                scene_id: ids.credits_scene.clone(),
                fade: Some(style.fade.clone()),
            },
        ]);
    }
    if let (Some(envelope), Some(id)) = (&settings.bgm_speech_envelope, &ids.bgm) {
        let clip = p.clips.iter().find(|c| &c.id == id).unwrap();
        let ClipContent::Audio { asset_id, playback } = &clip.content else {
            unreachable!()
        };
        let speech: Vec<_> = p
            .clips
            .iter()
            .filter(|c| matches!(c.content, ClipContent::Speech { .. }))
            .collect();
        let first = speech.iter().map(|c| c.start_us.0).min().unwrap();
        let last = speech
            .iter()
            .map(|c| c.start_us.0 + c.duration_us.0)
            .max()
            .unwrap();
        let duration = clip.start_us.0 + clip.duration_us.0;
        let mut playback = playback.clone();
        // The speech envelope and the clip's entrance fade are independent gains.
        playback.fade_in_us.0 = playback.fade_in_us.0.min(duration);
        playback.fade_out_us.0 = playback.fade_out_us.0.min(duration);
        playback.volume_points = envelope_points(first, last, duration, envelope);
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
                asset_id: asset_id.clone(),
                playback,
            },
        ]);
    }
    Ok(commands)
}

fn envelope_points(
    first: u64,
    last: u64,
    duration: u64,
    e: &BuildSpeechEnvelope,
) -> Vec<VolumePoint> {
    let mut points: Vec<VolumePoint> = vec![];
    for (at, level) in [
        (0, 1.),
        (first.saturating_sub(e.ramp_us.0), 1.),
        (first, e.speech_level),
        (last, e.speech_level),
        (last.saturating_add(e.ramp_us.0).min(duration), 1.),
        (duration, 1.),
    ] {
        if let Some(previous) = points.last_mut().filter(|p| p.at_us.0 == at) {
            previous.level = level;
        } else {
            points.push(VolumePoint {
                at_us: TimeUs(at),
                level,
            });
        }
    }
    points
}
