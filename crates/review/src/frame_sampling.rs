//! Plans representative frames; selecting a frame is not a visual inspection.
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::{Project, TimeUs};
use sungeki_scene::first_frame_at_or_after;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SampleReason {
    Start,
    End,
    SceneMiddle { scene_id: String },
    BeforeSceneStart { scene_id: String },
    SceneStart { scene_id: String },
    BeforeSceneEnd { scene_id: String },
    SceneEnd { scene_id: String },
    Requested { time_us: TimeUs },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameSample {
    pub frame_index: u64,
    pub time_us: TimeUs,
    pub reasons: Vec<SampleReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnsampledSpan {
    /// Half-open frame range, exact even at fractional FPS.
    pub first: u64,
    pub end: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameSampling {
    pub frame_count: u64,
    pub selected: Vec<FrameSample>,
    /// Candidate frames omitted by the limit, including their reasons.
    pub omitted: Vec<FrameSample>,
    /// All frames outside `selected`; selected frames are also uninspected until rendered/reviewed.
    pub unsampled: Vec<UnsampledSpan>,
    pub scenes_without_visible_frame: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameSamplingError(pub String);
fn error(message: &str) -> FrameSamplingError {
    FrameSamplingError(message.into())
}

/// At most 120 requested times and 1..=120 selected frames. Requested positions
/// have priority, then scene middles, then boundaries. Each tier is spread across
/// its chronological candidates when the remaining capacity cannot fit them all.
/// Only timing is inspected here; unrelated structural errors remain review issues.
pub fn plan_review_frames(
    project: &Project,
    requested: &[TimeUs],
    limit: usize,
) -> Result<FrameSampling, FrameSamplingError> {
    if !(1..=120).contains(&limit) || requested.len() > 120 {
        return Err(error("代表画像の件数は1〜120、指定時刻は120件以内です"));
    }
    let duration = project
        .duration()
        .ok_or_else(|| error("作品尺が不正です"))?;
    let fps = &project.canvas.fps;
    let at =
        |time| first_frame_at_or_after(fps, time).ok_or_else(|| error("フレーム時刻が不正です"));
    let frame_count = at(duration)?;
    let mut candidates: BTreeMap<u64, Vec<SampleReason>> = BTreeMap::new();
    let mut add = |index, reason| {
        if index < frame_count {
            let reasons = candidates.entry(index).or_default();
            if !reasons.contains(&reason) {
                reasons.push(reason);
            }
        }
    };
    if frame_count > 0 {
        add(0, SampleReason::Start);
        add(frame_count - 1, SampleReason::End);
    }
    for time in requested {
        let index = at(*time)?;
        if time.0 >= duration.0 || index >= frame_count {
            return Err(error("指定時刻に作品内のフレームがありません"));
        }
        add(index, SampleReason::Requested { time_us: *time });
    }
    let mut scenes_without_visible_frame = Vec::new();
    for scene in &project.scenes {
        let first = at(scene.start_us)?;
        let end_us = scene
            .start_us
            .0
            .checked_add(scene.duration_us.0)
            .ok_or_else(|| error("シーン尺が不正です"))?;
        let end = at(TimeUs(end_us))?;
        if first < end {
            let middle = at(TimeUs(scene.start_us.0 + scene.duration_us.0 / 2))?.min(end - 1);
            add(
                middle,
                SampleReason::SceneMiddle {
                    scene_id: scene.id.clone(),
                },
            );
        } else {
            scenes_without_visible_frame.push(scene.id.clone());
        }
        if first > 0 {
            add(
                first - 1,
                SampleReason::BeforeSceneStart {
                    scene_id: scene.id.clone(),
                },
            );
        }
        add(
            first,
            SampleReason::SceneStart {
                scene_id: scene.id.clone(),
            },
        );
        if end > 0 {
            add(
                end - 1,
                SampleReason::BeforeSceneEnd {
                    scene_id: scene.id.clone(),
                },
            );
        }
        add(
            end,
            SampleReason::SceneEnd {
                scene_id: scene.id.clone(),
            },
        );
    }
    let mut selected = BTreeSet::new();
    for priority in 0..3 {
        let tier: Vec<_> = candidates
            .iter()
            .filter(|(index, reasons)| {
                !selected.contains(*index)
                    && reasons.iter().any(|r| match (priority, r) {
                        (0, SampleReason::Requested { .. })
                        | (1, SampleReason::SceneMiddle { .. }) => true,
                        (2, _) => true,
                        _ => false,
                    })
            })
            .map(|(index, _)| *index)
            .collect();
        let count = (limit - selected.len()).min(tier.len());
        if count == 1 {
            selected.insert(tier[tier.len() / 2]);
        } else if count > 1 {
            for i in 0..count {
                selected.insert(tier[i * (tier.len() - 1) / (count - 1)]);
            }
        }
    }
    let mut result = FrameSampling {
        frame_count,
        selected: vec![],
        omitted: vec![],
        unsampled: vec![],
        scenes_without_visible_frame,
    };
    for (index, reasons) in candidates {
        let sample = FrameSample {
            frame_index: index,
            time_us: fps
                .frame_time(index)
                .ok_or_else(|| error("フレーム時刻が不正です"))?,
            reasons,
        };
        if selected.contains(&index) {
            result.selected.push(sample);
        } else {
            result.omitted.push(sample);
        }
    }
    let mut first = 0;
    for index in selected {
        if first < index {
            result.unsampled.push(UnsampledSpan { first, end: index });
        }
        first = index + 1;
    }
    if first < frame_count {
        result.unsampled.push(UnsampledSpan {
            first,
            end: frame_count,
        });
    }
    Ok(result)
}
