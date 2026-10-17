use sungeki_project::*;
use sungeki_scene::*;
fn fixture() -> Project {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let t = &mut p.clips[0].transform;
    t.motion = Some(VisualMotion {
        end_x: t.x + 100.,
        end_y: t.y + 50.,
        end_scale: 2.,
        easing: MotionEasing::Linear,
    });
    t.fade = Some(VisualFade {
        in_us: TimeUs(500_000),
        out_us: TimeUs(0),
    });
    p
}
#[test]
fn motion_and_fade_keep_parent_clock_across_mouth_boundaries() {
    let p = fixture();
    let original = p.clone();
    let plan = ScenePlan::new(&p).unwrap();
    let t = &p.clips[0].transform;
    for index in [0, 3, 6, 9, 15, 29] {
        let time = p.canvas.fps.frame_time(index).unwrap();
        let pfrac = time.0 as f64 / 1_000_000.;
        let layers = plan.frame(index).unwrap();
        let layer = layers.iter().find(|l| l.clip_id == p.clips[0].id).unwrap();
        assert!((layer.transform.x - (t.x + 100. * pfrac)).abs() < 1e-6);
        assert!((layer.transform.y - (t.y + 50. * pfrac)).abs() < 1e-6);
        assert!((layer.transform.width - t.width * (1. + pfrac)).abs() < 1e-6);
        assert!((layer.transform.opacity - t.opacity * (pfrac * 2.).min(1.)).abs() < 1e-6);
        assert!(layer.transform.motion.is_none());
    }
    assert_eq!(p, original);
}
#[test]
fn easing_endpoints_and_invalid_geometry() {
    let mut p = fixture();
    let t = p.clips[0].transform.clone();
    let mut m = t.motion.clone().unwrap();
    m.easing = MotionEasing::EaseInOut;
    for (elapsed, progress) in [
        (0, 0.),
        (250_000, 0.1464466094),
        (500_000, 0.5),
        (1_000_000, 1.),
    ] {
        assert!(
            (m.sample(&t, TimeUs(elapsed), TimeUs(1_000_000)).x - t.x - 100. * progress).abs()
                < 1e-6
        );
    }
    for scale in [0., -1., f64::NAN, f64::INFINITY, 1e20] {
        m.end_scale = scale;
        assert!(m.validate(&t).is_err());
    }
    p.clips[0].duration_us = TimeUs(10_000_000_000);
    p.scenes.clear();
    p.groups.clear();
    for c in &mut p.clips {
        c.scene_id = None;
    }
    assert!(ScenePlan::new(&p).is_err());
}
