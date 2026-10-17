//! Sample only animated portions at output frame timestamps. Static holds stay
//! single spans; PSD mouth boundaries retain the parent clip's fade clock.
use crate::{Layer, first_frame_at_or_after};
use std::collections::{BTreeSet, HashMap};
use sungeki_project::*;

pub(crate) fn expand(project: &Project, layers: Vec<Layer>) -> Result<Vec<Layer>, ValidationError> {
    let clips: HashMap<_, _> = project.clips.iter().map(|c| (c.id.as_str(), c)).collect();
    let fps = &project.canvas.fps;
    let mut result = Vec::new();
    for layer in layers {
        let clip = clips[layer.clip_id.as_str()];
        let fade = clip.transform.fade.as_ref();
        let motion = clip.transform.motion.as_ref();
        if fade.is_none() && motion.is_none() {
            result.push(layer);
            continue;
        }
        let mut boundaries = BTreeSet::from([layer.start_us.0, layer.end_us.0]);
        let end = clip.start_us.0 + clip.duration_us.0;
        let intervals = if motion.is_some() {
            vec![(clip.start_us.0, end)]
        } else {
            let fade = fade.unwrap();
            vec![
                (clip.start_us.0, clip.start_us.0 + fade.in_us.0),
                (end - fade.out_us.0, end),
            ]
        };
        for (start, stop) in intervals {
            let start = start.max(layer.start_us.0);
            let stop = stop.min(layer.end_us.0);
            if start >= stop {
                continue;
            }
            let first = first_frame_at_or_after(fps, TimeUs(start)).unwrap();
            let last = first_frame_at_or_after(fps, TimeUs(stop)).unwrap();
            if last - first + boundaries.len() as u64 + result.len() as u64 > 100_000 {
                return Err(ValidationError::new(
                    "映像効果の描画区間が100,000を超えています".into(),
                ));
            }
            for index in first..=last {
                if let Some(time) = fps.frame_time(index) {
                    if layer.start_us < time && time < layer.end_us {
                        boundaries.insert(time.0);
                    }
                }
            }
        }
        let boundaries: Vec<_> = boundaries.into_iter().collect();
        for window in boundaries.windows(2) {
            let first = first_frame_at_or_after(fps, TimeUs(window[0])).unwrap();
            let time = fps.frame_time(first).unwrap_or(TimeUs(window[1]));
            // Preserve arbitrary-time lookups (including PSD preview) even
            // when no output frame belongs to this sub-frame interval.
            let time = if time.0 >= window[1] {
                TimeUs(window[0])
            } else {
                time
            };
            let mut sampled = layer.clone();
            sampled.start_us = TimeUs(window[0]);
            sampled.end_us = TimeUs(window[1]);
            let elapsed = TimeUs(time.0 - clip.start_us.0);
            if let Some(motion) = motion {
                sampled.transform = motion.sample(&sampled.transform, elapsed, clip.duration_us);
            }
            if let Some(fade) = fade {
                sampled.transform.opacity *= fade.factor(elapsed, clip.duration_us);
            }
            sampled.transform.fade = None;
            result.push(sampled);
            if result.len() > 100_000 {
                return Err(ValidationError::new(
                    "描画区間が100,000を超えています".into(),
                ));
            }
        }
    }
    Ok(result)
}
