//! Expand a standing into disjoint static poses at source-sample boundaries.
use crate::{Layer, Primitive};
use sungeki_project::*;

pub(crate) fn layers(
    project: &Project,
    standing: &Clip,
    asset_id: &str,
    pose: &PsdPose,
    binding: &PsdSpeechBinding,
) -> Result<Vec<Layer>, ValidationError> {
    Ok(
        crate::mouth_timing::spans(project, standing, &binding.character_id)?
            .into_iter()
            .map(|(start_us, end_us, open)| Layer {
                clip_id: standing.id.clone(),
                scene_id: standing.scene_id.clone(),
                start_us,
                end_us,
                transform: standing.transform.clone(),
                primitive: Primitive::Psd {
                    asset_id: asset_id.into(),
                    pose: binding.pose(pose, open),
                },
            })
            .collect(),
    )
}
