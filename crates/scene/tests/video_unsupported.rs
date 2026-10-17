use sungeki_project::{ClipContent, Project, TimeUs};
use sungeki_scene::{Primitive, ScenePlan};
fn project() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap()
}
#[test]
fn video_source_positions_are_sampled_each_output_frame() {
    let mut p = project();
    let ClipContent::Video { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.looped = true;
    playback.source_duration_us = TimeUs(200_000);
    let plan = ScenePlan::new(&p).unwrap();
    assert_eq!(plan.layers.len() as u64, plan.frame_count);
    for frame in 0..plan.frame_count {
        let layers = plan.frame(frame).unwrap();
        assert_eq!(layers.len(), 1);
        let Primitive::Video { source_time_us, .. } = layers[0].primitive else {
            panic!()
        };
        assert_eq!(
            source_time_us,
            TimeUs(200_000 + p.canvas.fps.frame_time(frame).unwrap().0 % 200_000)
        );
        assert_eq!(layers[0].scene_id, p.clips[0].scene_id);
    }
    let spans: Vec<_> = plan.visual_spans().collect();
    assert_eq!(spans.len() as u64, plan.frame_count);
    assert!(spans.iter().all(|s| s.frames.end == s.frames.first + 1));
    p.tracks[0].visible = false;
    assert!(ScenePlan::new(&p).unwrap().layers.is_empty());
}
#[test]
fn per_frame_source_positions_survive_visual_fade_and_motion_expansion() {
    let mut p = project();
    p.clips[0].transform.fade = Some(sungeki_project::VisualFade {
        in_us: TimeUs(100_000),
        out_us: TimeUs(100_000),
    });
    let plan = ScenePlan::new(&p).unwrap();
    for frame in 0..plan.frame_count {
        let layers = plan.frame(frame).unwrap();
        assert_eq!(layers.len(), 1);
        let Primitive::Video { source_time_us, .. } = layers[0].primitive else {
            panic!()
        };
        assert_eq!(
            source_time_us,
            TimeUs(200_000 + p.canvas.fps.frame_time(frame).unwrap().0)
        );
    }
}
