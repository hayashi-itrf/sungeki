//! Formal text layout across the scene's evaluated text intervals, without
//! capturing unrelated image/audio originals or retaining raster intermediates.
use crate::{RenderError, RenderRuntime, frame_cache::FrameCache, originals, plan::error};
use std::{fs, path::Path, sync::atomic::AtomicBool, time::Instant};
use sungeki_project::{Project, TimeUs};
use sungeki_scene::{Primitive, ScenePlan, first_frame_at_or_after};

pub struct TextObservation {
    pub clip_id: String,
    pub frame_index: u64,
    pub end_us: TimeUs,
    pub error: Option<RenderError>,
    pub margin_issues: Vec<crate::TextLayoutIssue>,
}
pub struct TextInspection {
    pub observations: Vec<TextObservation>,
    pub total_intervals: usize,
    pub omitted_details: usize,
}
/// The caller owns a new, empty private directory. Every interval containing an
/// output frame uses the movie worker/font/layout. Safety margins are opt-in;
/// occlusion and readability are not tested. At most 1000 intervals are attempted, within one deadline.
pub fn inspect_text_layout(
    project: &Project,
    margin_percent: u8,
    runtime: &RenderRuntime,
    directory: &Path,
    cancelled: &AtomicBool,
) -> Result<TextInspection, RenderError> {
    if margin_percent > 25 {
        return Err(error("安全余白は0〜25%で指定してください"));
    }
    let runner = crate::prepare::runner(runtime, directory, cancelled)?;
    let scene = ScenePlan::new(project).map_err(error)?;
    let intervals = intervals(&scene);
    let mut result = TextInspection {
        observations: Vec::new(),
        total_intervals: intervals.len(),
        omitted_details: 0,
    };
    if intervals.is_empty() {
        return Ok(result);
    }
    originals::write_new(&directory.join("resources.json"), b"{}")?;
    originals::write_new(
        &directory.join("font.ttc"),
        &originals::read_verified(&runtime.font, &runtime.font_sha256, None)?,
    )?;
    fs::create_dir(directory.join("psd-cache"))?;
    let mut detail_budget = 1000usize;
    for (index, (layer, frame)) in intervals.into_iter().take(1000).enumerate() {
        originals::cancelled(cancelled)?;
        if Instant::now() >= runner.end {
            break;
        }
        // A fresh cache keeps overflow diagnostics associated with this clip.
        let rendered = FrameCache::default().frame(&runner, &scene.canvas, &[layer], index, frame);
        originals::cancelled(cancelled)?;
        let mut margin_issues = Vec::new();
        let rendered = rendered.and_then(|path| {
            margin_issues = crate::text_margin::inspect(
                &runner,
                &scene.canvas,
                layer,
                index + 1000,
                frame,
                margin_percent,
            )?;
            Ok(path)
        });
        let rendered = rendered.and_then(|path| {
            if Instant::now() >= runner.end {
                Err(error("文字検査の実行期限を超えました"))
            } else {
                Ok(path)
            }
        });
        let failure = rendered.err().map(|mut error| {
            if let RenderError::TextLayout(issues) = &mut error {
                let kept = issues.len().min(detail_budget);
                result.omitted_details += issues.len() - kept;
                issues.truncate(kept);
                detail_budget -= kept;
            }
            error
        });
        let kept = margin_issues.len().min(detail_budget);
        result.omitted_details += margin_issues.len() - kept;
        margin_issues.truncate(kept);
        detail_budget -= kept;
        result.observations.push(TextObservation {
            clip_id: layer.clip_id.clone(),
            frame_index: frame,
            end_us: layer.end_us,
            error: failure,
            margin_issues,
        });
        // These are private intermediates, never exposed as representative PNGs.
        for name in [
            format!("frame-{index}.png"),
            format!("frame-{index}.json"),
            format!("report-{index}.json"),
            format!("frame-{}.png", index + 1000),
            format!("frame-{}.json", index + 1000),
            format!("report-{}.json", index + 1000),
        ] {
            match fs::remove_file(directory.join(name)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    originals::cancelled(cancelled)?;
    Ok(result)
}

fn intervals(scene: &ScenePlan) -> Vec<(&sungeki_scene::Layer, u64)> {
    scene
        .layers
        .iter()
        .filter_map(|layer| {
            if !matches!(layer.primitive, Primitive::Text { .. }) {
                return None;
            }
            let frame = first_frame_at_or_after(&scene.canvas.fps, layer.start_us)?;
            let time = scene.canvas.fps.frame_time(frame)?;
            (time < layer.end_us).then_some((layer, frame))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sungeki_project::*;
    #[test]
    fn every_text_clip_is_planned_even_outside_representative_times() {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        p.clips[0].content = ClipContent::Text {
            outline: None,
            layout: None,
            text: "文字".into(),
            font_size: 20.,
            color: "#000000".into(),
        };
        p.clips[0].scene_id = None;
        let mut late = p.clips[0].clone();
        late.id = uuid::Uuid::new_v4().to_string();
        late.start_us = TimeUs(5_000_001);
        late.duration_us = TimeUs(100_000);
        p.clips.push(late);
        let mut hidden = p.clips[0].clone();
        let mut track = p.tracks[0].clone();
        track.id = uuid::Uuid::new_v4().to_string();
        track.visible = false;
        hidden.id = uuid::Uuid::new_v4().to_string();
        hidden.track_id = track.id.clone();
        p.tracks.push(track);
        p.clips.push(hidden);
        let mut subframe = p.clips[0].clone();
        subframe.id = uuid::Uuid::new_v4().to_string();
        subframe.start_us = TimeUs(1);
        subframe.duration_us = TimeUs(1);
        p.clips.push(subframe);
        let scene = ScenePlan::new(&p).unwrap();
        let selected = intervals(&scene);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].1, 0);
        assert_eq!(selected[1].1, 151);
        assert_eq!(selected[1].0.clip_id, p.clips[1].id);
        p.clips[0].transform.fade = Some(VisualFade {
            in_us: TimeUs(100_000),
            out_us: TimeUs(100_000),
        });
        let scene = ScenePlan::new(&p).unwrap();
        assert!(intervals(&scene).len() > 2);
    }
}
