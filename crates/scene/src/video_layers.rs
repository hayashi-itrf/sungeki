use crate::{Layer, Primitive, VideoClipTiming, first_frame_at_or_after};
use sungeki_project::*;
pub(crate) fn append(
    project: &Project,
    clip: &Clip,
    result: &mut Vec<Layer>,
) -> Result<(), ValidationError> {
    let ClipContent::Video { asset_id, .. } = &clip.content else {
        unreachable!()
    };
    let asset = project
        .assets
        .iter()
        .find(|a| a.id == *asset_id)
        .ok_or_else(|| ValidationError::new("動画素材がありません".into()))?;
    let AssetProbe::Video(probe) = &asset.probe else {
        return Err(ValidationError::new(
            "動画素材の検査情報がありません".into(),
        ));
    };
    let timing = VideoClipTiming::new(clip, probe)?;
    let fps = &project.canvas.fps;
    let end = TimeUs(clip.start_us.0 + clip.duration_us.0);
    let first = first_frame_at_or_after(fps, clip.start_us).unwrap();
    let last = first_frame_at_or_after(fps, end).unwrap();
    if last - first + result.len() as u64 > 100_000 {
        return Err(ValidationError::new(
            "動画の描画区間が100,000を超えています".into(),
        ));
    }
    for frame in first..last {
        let time = fps.frame_time(frame).unwrap();
        result.push(Layer {
            clip_id: clip.id.clone(),
            scene_id: clip.scene_id.clone(),
            start_us: time,
            end_us: fps.frame_time(frame + 1).unwrap().min(end),
            transform: clip.transform.clone(),
            primitive: Primitive::Video {
                asset_id: asset_id.clone(),
                source_time_us: timing.source_time(time).unwrap(),
            },
        });
    }
    Ok(())
}
