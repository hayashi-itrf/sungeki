//! Sprite evaluation produces ordinary image primitives for the common renderer.
use crate::{Layer, Primitive};
use sungeki_project::*;

/// Evaluate a registered standing's sprite artwork without modifying the project.
/// Validates external input; ScenePlan uses the already-validated internal path.
pub fn sprite_layers(
    project: &Project,
    standing_id: &str,
    sprites: &SpriteSet,
    pose_name: Option<&str>,
    character_id: Option<&str>,
) -> Result<Vec<Layer>, ValidationError> {
    validate(project)?;
    sprites.validate(&project.assets)?;
    sprites.pose(pose_name)?;
    let standing = project
        .clips
        .iter()
        .find(|clip| clip.id == standing_id)
        .ok_or_else(|| ValidationError::new("スプライトの配置クリップがありません".into()))?;
    let track = project
        .tracks
        .iter()
        .find(|track| track.id == standing.track_id)
        .ok_or_else(|| ValidationError::new("スプライトの配置トラックがありません".into()))?;
    if track.kind != TrackKind::Visual {
        return Err(ValidationError::new(
            "スプライトは映像トラックに配置してください".into(),
        ));
    }
    if let Some(id) = character_id {
        if !project
            .characters
            .iter()
            .any(|character| character.id == id)
        {
            return Err(ValidationError::new(
                "口パク対象のキャラクターがありません".into(),
            ));
        }
    }
    if !track.visible {
        return Ok(vec![]);
    }
    layers(project, standing, sprites, pose_name, character_id)
}

pub(crate) fn layers(
    project: &Project,
    standing: &Clip,
    sprites: &SpriteSet,
    pose_name: Option<&str>,
    character_id: Option<&str>,
) -> Result<Vec<Layer>, ValidationError> {
    let pose = sprites.pose(pose_name)?;
    let spans = match character_id.filter(|_| pose.open_asset_id.is_some()) {
        Some(id) => crate::mouth_timing::spans(project, standing, id)?,
        None => vec![(
            standing.start_us,
            TimeUs(standing.start_us.0 + standing.duration_us.0),
            false,
        )],
    };
    spans
        .into_iter()
        .map(|(start_us, end_us, open)| {
            Ok(Layer {
                clip_id: standing.id.clone(),
                scene_id: standing.scene_id.clone(),
                start_us,
                end_us,
                transform: standing.transform.clone(),
                primitive: Primitive::Image {
                    asset_id: sprites.asset_id(pose_name, open)?.into(),
                },
            })
        })
        .collect()
}
