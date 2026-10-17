//! Speech-relative images, video diagrams and image reactions.
use crate::{Command, build_visuals as visual};
use sungeki_project::*;
pub(super) fn plan(
    p: &Project,
    scene: &BuildScene,
    source: &Scene,
    diagram_ids: &[String],
    offset: usize,
    track_id: &str,
) -> Result<Vec<Command>, ValidationError> {
    let mut commands = Vec::new();
    for (diagram, id) in scene.diagrams.iter().zip(diagram_ids) {
        let first = &p.clips[offset + usize::from(diagram.first_line)];
        let last = &p.clips[offset + usize::from(diagram.last_line)];
        let mut transform = visual::diagram_rect(p, &diagram.asset_id)?;
        if let Some(motion) = &diagram.motion {
            motion.validate(&transform)?;
            transform.motion = Some(motion.clone());
        }
        let duration = TimeUs(last.start_us.0 + last.duration_us.0 - first.start_us.0);
        if let Some(fade) = &diagram.fade {
            fade.validate(duration)
                .map_err(|e| ValidationError::new(format!("説明図の合成後の表示時間: {}", e.0)))?;
            transform.fade = Some(fade.clone());
        }
        commands.push(Command::AddClip {
            clip: Clip {
                id: id.clone(),
                track_id: track_id.to_owned(),
                scene_id: Some(source.id.clone()),
                start_us: TimeUs(first.start_us.0 + 3_000_000),
                duration_us: duration,
                content: match &diagram.video_playback {
                    Some(playback) => ClipContent::Video {
                        asset_id: diagram.asset_id.clone(),
                        playback: playback.clone(),
                    },
                    None => ClipContent::Image {
                        asset_id: diagram.asset_id.clone(),
                    },
                },
                transform,
            },
        });
        if let Some(reaction) = &diagram.reaction {
            commands.push(Command::SetReactionMotion {
                clip_id: id.clone(),
                center_x: reaction.center_x,
                center_y: reaction.center_y,
                width: reaction.width,
                height: reaction.height,
                start_scale: reaction.start_scale,
                fade: reaction.fade.clone(),
            });
        }
    }
    Ok(commands)
}
