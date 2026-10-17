use sungeki_project::*;
use sungeki_scene::*;
fn fixture() -> Project {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    p.scenes.clear();
    p.groups.clear();
    for c in &mut p.clips {
        c.scene_id = None;
    }
    p.scenes.push(Scene {
        id: "00000000-0000-4000-8000-000000000099".into(),
        name: "全画面".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(3_000_000),
        fade: Some(VisualFade {
            in_us: TimeUs(1_000_000),
            out_us: TimeUs(1_000_000),
        }),
    });
    p
}
#[test]
fn composite_cover_is_last_leaves_layers_and_audio_intact() {
    let p = fixture();
    let plan = ScenePlan::new(&p).unwrap();
    let mut plain = p.clone();
    plain.scenes[0].fade = None;
    let original = ScenePlan::new(&plain).unwrap();
    assert_eq!(plan.audio, original.audio);
    for (us, alpha) in [(0, 1.), (500_000, 0.5), (2_500_000, 0.5)] {
        let layers: Vec<_> = plan.layers_at(TimeUs(us)).collect();
        let cover = layers.last().unwrap();
        assert!((cover.transform.opacity - alpha).abs() < 1e-6);
        assert!(
            matches!(&cover.primitive, Primitive::Shape { fill, .. } if fill == &p.canvas.background)
        );
        assert_eq!(
            &layers[..layers.len() - 1],
            original.layers_at(TimeUs(us)).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        plan.layers_at(TimeUs(1_500_000)).collect::<Vec<_>>(),
        original.layers_at(TimeUs(1_500_000)).collect::<Vec<_>>()
    );
    assert_eq!(
        plan.layers_at(TimeUs(3_000_000)).collect::<Vec<_>>(),
        original.layers_at(TimeUs(3_000_000)).collect::<Vec<_>>()
    );
}
#[test]
fn invalid_and_huge_fades_fail_before_allocating_frames() {
    let mut p = fixture();
    p.scenes[0].duration_us = TimeUs(1);
    assert!(ScenePlan::new(&p).is_err());
    p.scenes[0].duration_us = TimeUs(10_000_000_000);
    p.scenes[0].fade.as_mut().unwrap().in_us = TimeUs(9_000_000_000);
    assert!(ScenePlan::new(&p).is_err());
}
