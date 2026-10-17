use crate::{Command, apply_batch};
use sungeki_project::{ClipContent, Project, SubtitleOutline, ValidationError, validate};

/// Fit the complete old canvas inside the new one, centered, without stretching.
/// Returns ordinary editor commands so callers keep one atomic edit and Undo.
/// Does not reflow a dialogue or crop content to fill a different aspect ratio.
pub fn plan_resize_layout(
    project: &Project,
    width: u32,
    height: u32,
) -> Result<Vec<Command>, ValidationError> {
    validate(project)?;
    let mut canvas = project.canvas.clone();
    canvas.width = width;
    canvas.height = height;
    let mut probe = project.clone();
    probe.canvas = canvas.clone();
    validate(&probe)?;
    let scale = (width as f64 / project.canvas.width as f64)
        .min(height as f64 / project.canvas.height as f64);
    let dx = (width as f64 - project.canvas.width as f64 * scale) / 2.;
    let dy = (height as f64 - project.canvas.height as f64 * scale) / 2.;
    let mut commands = vec![Command::SetCanvas { canvas }];
    for character in &project.characters {
        let mut character = character.clone();
        character.subtitle_style.font_size *= scale;
        scale_outline(&mut character.subtitle_style.outline, scale)?;
        commands.push(Command::SetCharacter { character });
    }
    for clip in &project.clips {
        if matches!(clip.content, ClipContent::Audio { .. }) {
            continue;
        }
        let mut transform = clip.transform.clone();
        transform.x = transform.x * scale + dx;
        transform.y = transform.y * scale + dy;
        transform.width *= scale;
        transform.height *= scale;
        if let Some(motion) = &mut transform.motion {
            motion.end_x = motion.end_x * scale + dx;
            motion.end_y = motion.end_y * scale + dy;
        }
        commands.push(Command::SetTransform {
            clip_id: clip.id.clone(),
            transform,
        });
        match &clip.content {
            ClipContent::Text {
                font_size,
                color,
                outline,
                ..
            } => {
                let mut outline = outline.clone();
                scale_outline(&mut outline, scale)?;
                commands.push(Command::SetTextStyle {
                    clip_id: clip.id.clone(),
                    font_size: font_size * scale,
                    color: color.clone(),
                });
                commands.push(Command::SetTextOutline {
                    clip_id: clip.id.clone(),
                    outline,
                });
            }
            ClipContent::Blur { sigma } => commands.push(Command::SetBlur {
                clip_id: clip.id.clone(),
                sigma: sigma * scale,
            }),
            _ => {}
        }
    }
    // Includes renderer/model bounds on motion, blur, geometry and outline.
    // Never return a partially usable plan when the new size exceeds them.
    apply_batch(project, &commands)?;
    Ok(commands)
}
fn scale_outline(outline: &mut Option<SubtitleOutline>, scale: f64) -> Result<(), ValidationError> {
    if let Some(outline) = outline {
        let width = (outline.width as f64 * scale).round().max(1.);
        if width > 32. {
            return Err(ValidationError::new(
                "拡大後の字幕の縁取りが32pxを超えます".into(),
            ));
        }
        outline.width = width as u32;
    }
    Ok(())
}
