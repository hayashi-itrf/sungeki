use sungeki_project::{FrameRate, Project, Scene, TimeUs};
use sungeki_review::*;
fn project() -> Project {
    let mut p = Project::empty(
        "00000000-0000-4000-8000-000000000001".into(),
        "代表画像".into(),
    );
    p.scenes = (0..3)
        .map(|i| Scene {
            id: format!("scene-{i}"),
            name: format!("scene {i}"),
            start_us: TimeUs(i * 1_000_000),
            duration_us: TimeUs(1_000_000),
            fade: None,
        })
        .collect();
    p
}
#[test]
fn middles_boundaries_requests_and_unsampled_ranges_are_exact() {
    let p = project();
    let original = p.clone();
    let plan = plan_review_frames(&p, &[TimeUs(1_000_001)], 120).unwrap();
    assert_eq!(p, original);
    assert_eq!(
        plan.selected
            .iter()
            .map(|s| s.frame_index)
            .collect::<Vec<_>>(),
        [0, 15, 29, 30, 31, 45, 59, 60, 75, 89]
    );
    assert_eq!(plan.frame_count, 90);
    assert!(plan.omitted.is_empty());
    let boundary = plan.selected.iter().find(|s| s.frame_index == 30).unwrap();
    assert!(boundary.reasons.contains(&SampleReason::SceneStart {
        scene_id: "scene-1".into()
    }));
    assert!(boundary.reasons.contains(&SampleReason::SceneEnd {
        scene_id: "scene-0".into()
    }));
    // Every frame is exactly one of planned or unsampled, no gaps/overlap.
    for frame in 0..90 {
        let selected = plan
            .selected
            .iter()
            .filter(|s| s.frame_index == frame)
            .count();
        let unsampled = plan
            .unsampled
            .iter()
            .filter(|s| s.first <= frame && frame < s.end)
            .count();
        assert_eq!(selected + unsampled, 1);
    }
}
#[test]
fn capped_selection_prioritizes_requests_then_spreads_scene_middles() {
    let p = project();
    let plan = plan_review_frames(&p, &[TimeUs(10), TimeUs(10)], 3).unwrap();
    assert_eq!(
        plan.selected
            .iter()
            .map(|s| s.frame_index)
            .collect::<Vec<_>>(),
        [1, 15, 75]
    );
    assert!(!plan.omitted.is_empty());
    assert_eq!(plan.selected[0].reasons.len(), 1);
    let reordered = Project {
        scenes: p.scenes.iter().cloned().rev().collect(),
        ..p
    };
    let next = plan_review_frames(&reordered, &[TimeUs(10)], 3).unwrap();
    assert_eq!(plan.selected, next.selected);
    let limited = plan_review_frames(
        &project(),
        &[TimeUs(0), TimeUs(1_000_000), TimeUs(2_000_000)],
        1,
    )
    .unwrap();
    assert_eq!(limited.selected[0].frame_index, 30);
    assert!(limited.omitted.iter().any(|s| {
        s.reasons
            .contains(&SampleReason::Requested { time_us: TimeUs(0) })
    }));
}
#[test]
fn fractional_fps_half_open_end_and_subframe_scenes() {
    let mut p = project();
    p.canvas.fps = FrameRate {
        numerator: 30_000,
        denominator: 1001,
    };
    p.scenes.truncate(1);
    p.scenes[0].start_us = TimeUs(100);
    p.scenes[0].duration_us = TimeUs(100);
    let plan = plan_review_frames(&p, &[], 120).unwrap();
    assert_eq!(plan.frame_count, 1);
    assert_eq!(plan.selected[0].frame_index, 0);
    assert_eq!(plan.scenes_without_visible_frame, ["scene-0"]);
    assert!(plan.unsampled.is_empty());
    // There is no render frame at/after this in-range time before the duration ends.
    assert!(plan_review_frames(&p, &[TimeUs(150)], 120).is_err());
    p = project();
    p.canvas.fps = FrameRate {
        numerator: 30_000,
        denominator: 1001,
    };
    let plan = plan_review_frames(&p, &[TimeUs(1_001_001)], 120).unwrap();
    assert!(
        plan.selected
            .iter()
            .any(|s| s.frame_index == 31 && s.time_us == TimeUs(1_034_366))
    );
    assert!(plan_review_frames(&p, &[TimeUs(3_000_000)], 120).is_err());
}
#[test]
fn empty_invalid_limits_and_overflow_are_explicit_without_claiming_a_check_passed() {
    let empty = Project::empty("fixture".into(), "empty".into());
    let plan = plan_review_frames(&empty, &[], 12).unwrap();
    assert_eq!(plan.frame_count, 0);
    assert!(plan.selected.is_empty());
    assert!(plan_review_frames(&empty, &[TimeUs(0)], 12).is_err());
    assert!(plan_review_frames(&empty, &[], 0).is_err());
    assert!(plan_review_frames(&empty, &[], 121).is_err());
    assert!(plan_review_frames(&empty, &vec![TimeUs(0); 121], 12).is_err());
    let mut p = project();
    p.canvas.fps.denominator = 0;
    assert!(plan_review_frames(&p, &[], 12).is_err());
    p = project();
    p.scenes[0].duration_us = TimeUs(u64::MAX);
    assert!(plan_review_frames(&p, &[], 12).is_err());
}
