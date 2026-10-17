use sungeki_project::*;
use sungeki_scene::*;
fn fixture() -> Project {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let mut p: Project = serde_json::from_value(value["project"].clone()).unwrap();
    p.scenes.clear();
    p.groups.clear();
    p.clips.truncate(1);
    let c = &mut p.clips[0];
    c.scene_id = None;
    c.start_us = TimeUs(0);
    c.duration_us = TimeUs(3_000_000);
    c.transform.opacity = 0.8;
    c.transform.fade = Some(VisualFade {
        in_us: TimeUs(1_000_000),
        out_us: TimeUs(1_000_000),
    });
    p.canvas.fps = FrameRate {
        numerator: 30,
        denominator: 1,
    };
    p
}
#[test]
fn samples_linear_fades_and_keeps_static_hold_compact() {
    let p = fixture();
    let before = p.clone();
    let plan = ScenePlan::new(&p).unwrap();
    for (index, opacity) in [(0, 0.), (15, 0.4), (30, 0.8), (60, 0.8), (75, 0.4)] {
        assert!((plan.frame(index).unwrap()[0].transform.opacity - opacity).abs() < 1e-6);
    }
    assert!(plan.layers.len() <= 62);
    assert_eq!(
        plan.visual_spans()
            .map(|s| s.frames.end - s.frames.first)
            .sum::<u64>(),
        90
    );
    assert_eq!(p, before);
    let roundtrip: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(p, roundtrip);
}
#[test]
fn rejects_overlap_and_oversized_expansion() {
    let mut p = fixture();
    p.clips[0].duration_us = TimeUs(1_000_000);
    assert!(validate(&p).is_err());
    p.clips[0].duration_us = TimeUs(10_000_000_000);
    p.clips[0].transform.fade.as_mut().unwrap().in_us = TimeUs(9_000_000_000);
    assert!(ScenePlan::new(&p).is_err());
}
#[test]
fn non_frame_aligned_start_uses_absolute_output_clock() {
    let mut p = fixture();
    p.clips[0].start_us = TimeUs(10_000);
    let plan = ScenePlan::new(&p).unwrap();
    assert!(plan.frame(0).unwrap().is_empty());
    for index in 1..90 {
        let layers = plan.frame(index).unwrap();
        assert_eq!(layers.len(), 1);
        let c = &p.clips[0];
        let elapsed = TimeUs(p.canvas.fps.frame_time(index).unwrap().0 - c.start_us.0);
        assert!(
            (layers[0].transform.opacity
                - c.transform.opacity
                    * c.transform
                        .fade
                        .as_ref()
                        .unwrap()
                        .factor(elapsed, c.duration_us))
            .abs()
                < 1e-10
        );
    }
}

#[test]
fn sub_frame_clips_remain_available_to_time_based_preview() {
    let mut p = fixture();
    p.clips[0].start_us = TimeUs(10_000);
    p.clips[0].duration_us = TimeUs(10_000);
    p.clips[0].transform.fade = Some(VisualFade {
        in_us: TimeUs(5_000),
        out_us: TimeUs(5_000),
    });
    let plan = ScenePlan::new(&p).unwrap();
    assert_eq!(plan.layers_at(TimeUs(15_000)).count(), 1);
    assert!(plan.frame(0).unwrap().is_empty());
}
