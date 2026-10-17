//! Frame-sampled VRM evaluation shared by output and prepared previews.
use crate::{Layer, Primitive, first_frame_at_or_after};
use sungeki_project::*;

pub(crate) fn append(
    project: &Project,
    clip: &Clip,
    result: &mut Vec<Layer>,
) -> Result<(), ValidationError> {
    let ClipContent::Vrm { standing } = &clip.content else {
        unreachable!()
    };
    let width = clip.transform.width.ceil() as u32;
    let height = clip.transform.height.ceil() as u32;
    if width > 2048 || height > 2048 {
        return Err(ValidationError::new("VRM描画寸法は2048pxまでです".into()));
    }
    let mouth = if let Some(character) = &standing.character_id {
        let supports_aa = project.assets.iter().find(|a| a.id == standing.model_asset_id)
            .is_some_and(|a| matches!(&a.probe, AssetProbe::Vrm(p) if p.inspection.expressions.iter().any(|name| name == "aa")));
        if !supports_aa {
            return Err(ValidationError::new(
                "VRMの音声連動にはaa表情が必要です".into(),
            ));
        }
        Some(crate::mouth_timing::spans(project, clip, character)?)
    } else {
        None
    };
    let fps = &project.canvas.fps;
    let end = TimeUs(clip.start_us.0 + clip.duration_us.0);
    let first = first_frame_at_or_after(fps, clip.start_us).unwrap();
    let last = first_frame_at_or_after(fps, end).unwrap();
    if last - first + result.len() as u64 > 100_000 {
        return Err(ValidationError::new(
            "VRMの描画区間が100,000を超えています".into(),
        ));
    }
    let mut cursor = 0;
    for frame in first..last {
        let time = fps.frame_time(frame).unwrap();
        let mut expressions = standing.expressions.clone();
        if let Some(spans) = &mouth {
            while cursor < spans.len() && spans[cursor].1 <= time {
                cursor += 1;
            }
            let open = spans
                .get(cursor)
                .is_some_and(|&(start, stop, open)| start <= time && time < stop && open);
            expressions.insert("aa".into(), if open { 1.0 } else { 0.0 });
        }
        result.push(Layer {
            clip_id: clip.id.clone(),
            scene_id: clip.scene_id.clone(),
            start_us: time,
            end_us: fps.frame_time(frame + 1).unwrap().min(end),
            transform: clip.transform.clone(),
            primitive: Primitive::Vrm {
                width,
                height,
                model_asset_id: standing.model_asset_id.clone(),
                motion_asset_id: standing.motion_asset_id.clone(),
                source_time_us: TimeUs(standing.source_start_us.0 + time.0 - clip.start_us.0),
                expressions,
                pose: standing.pose.clone().unwrap_or_default(),
            },
        });
    }
    Ok(())
}
