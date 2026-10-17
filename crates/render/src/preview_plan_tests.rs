use super::*;
use sungeki_project::*;
use sungeki_scene::Layer;
fn scene(width: u32, height: u32) -> ScenePlan {
    ScenePlan {
        canvas: Canvas {
            width,
            height,
            fps: FrameRate {
                numerator: 30000,
                denominator: 1001,
            },
            background: "#123456".into(),
        },
        duration_us: TimeUs(1_001_000),
        frame_count: 30,
        layers: vec![],
        audio: vec![],
        audio_mix: AudioMix {
            normalization: None,
        },
    }
}
fn layer(primitive: Primitive) -> Layer {
    Layer {
        clip_id: "clip".into(),
        scene_id: Some("scene".into()),
        start_us: TimeUs(1234),
        end_us: TimeUs(999999),
        transform: Transform {
            x: 100.,
            y: 200.,
            width: 600.,
            height: 800.,
            opacity: 0.7,
            flip_x: Some(true),
            motion: None,
            fade: None,
        },
        primitive,
    }
}
#[test]
fn preview_scales_geometry_text_blur_and_vrm_without_changing_clocks() {
    let mut value = scene(1920, 1080);
    value.layers = vec![
        layer(Primitive::Text {
            text: "字幕".into(),
            font_size: 40.,
            color: "#ffffff".into(),
            layout: None,
            outline: Some(SubtitleOutline {
                width: 4,
                color: "#000000".into(),
            }),
        }),
        layer(Primitive::Blur { sigma: 20. }),
        layer(Primitive::Vrm {
            width: 600,
            height: 800,
            model_asset_id: "model".into(),
            motion_asset_id: "motion".into(),
            source_time_us: TimeUs(123456),
            pose: Default::default(),
            expressions: [("aa".into(), 1.)].into(),
        }),
    ];
    let original = value.clone();
    shrink(&mut value);
    assert_eq!((value.canvas.width, value.canvas.height), (960, 540));
    assert_eq!(value.canvas.fps, original.canvas.fps);
    assert_eq!(
        (value.duration_us, value.frame_count),
        (original.duration_us, original.frame_count)
    );
    assert_eq!(value.audio, original.audio);
    assert_eq!(value.audio_mix, original.audio_mix);
    for (a, b) in value.layers.iter().zip(&original.layers) {
        assert_eq!(
            (&a.clip_id, &a.scene_id, a.start_us, a.end_us),
            (&b.clip_id, &b.scene_id, b.start_us, b.end_us)
        );
        assert_eq!(
            (
                a.transform.x,
                a.transform.y,
                a.transform.width,
                a.transform.height
            ),
            (50., 100., 300., 400.)
        );
        assert_eq!((a.transform.opacity, a.transform.flip_x), (0.7, Some(true)));
    }
    assert!(
        matches!(&value.layers[0].primitive, Primitive::Text { font_size, outline: Some(SubtitleOutline { width: 2, .. }), .. } if *font_size == 20.)
    );
    assert!(matches!(
        value.layers[1].primitive,
        Primitive::Blur { sigma: 10. }
    ));
    assert!(matches!(
        value.layers[2].primitive,
        Primitive::Vrm {
            width: 300,
            height: 400,
            source_time_us: TimeUs(123456),
            ..
        }
    ));
}
#[test]
fn portrait_irregular_and_small_canvases_remain_even_and_undistorted() {
    for (w, h) in [(1080, 1920), (4094, 4092), (4096, 2)] {
        let mut value = scene(w, h);
        value.layers.push(layer(Primitive::Shape {
            shape: ShapeKind::Rectangle,
            fill: "#ffffff".into(),
        }));
        shrink(&mut value);
        assert!(value.canvas.width <= 960 && value.canvas.height <= 960);
        assert_eq!((value.canvas.width % 2, value.canvas.height % 2), (0, 0));
        assert!(
            (value.layers[0].transform.width / value.layers[0].transform.height - 0.75).abs()
                < 1e-10
        );
    }
    let mut value = scene(640, 360);
    let original = value.clone();
    shrink(&mut value);
    assert_eq!(value, original);
}

#[test]
fn original_project_and_audio_scene_remain_unchanged() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let mut project: Project = serde_json::from_value(fixture["project"].clone()).unwrap();
    let full = ScenePlan::new(&project).unwrap();
    assert!(!full.audio.is_empty());
    let mut reduced = full.clone();
    shrink(&mut reduced);
    assert_eq!(reduced.audio, full.audio);
    assert_eq!(reduced.audio_mix, full.audio_mix);
    project.clips.retain(|c| {
        matches!(
            c.content,
            ClipContent::Text { .. } | ClipContent::Shape { .. }
        )
    });
    project.groups.clear();
    project.scenes.clear();
    for clip in &mut project.clips {
        clip.scene_id = None;
    }
    let before = serde_json::to_value(&project).unwrap();
    let plan = PreviewPlan::new(&project, |_| panic!("no speech in this fixture")).unwrap();
    assert_eq!(plan.source_dimensions(), (1920, 1080));
    assert_eq!(plan.dimensions(), (960, 540));
    assert_eq!(serde_json::to_value(&project).unwrap(), before);
}

#[test]
fn thin_subtitle_outline_remains_valid_after_four_k_reduction() {
    let mut value = scene(3840, 2160);
    value.layers.push(layer(Primitive::Text {
        text: "細い輪郭".into(),
        font_size: 40.,
        color: "#ffffff".into(),
        layout: None,
        outline: Some(SubtitleOutline {
            width: 1,
            color: "#000000".into(),
        }),
    }));
    shrink(&mut value);
    assert!(matches!(&value.layers[0].primitive,
        Primitive::Text { font_size, outline: Some(SubtitleOutline { width: 1, .. }), .. } if *font_size == 10.));
}

#[test]
fn tiny_text_remains_renderable_without_changing_formal_font_sizes() {
    for (canvas, font, expected) in [
        (3840, 1., 1.),
        (3840, 2., 1.),
        (3840, 6., 1.5),
        (8192, 1., 1.),
    ] {
        let mut value = scene(canvas, 2160);
        value.layers.push(layer(Primitive::Text {
            text: "微小文字".into(),
            font_size: font,
            color: "#ffffff".into(),
            layout: None,
            outline: None,
        }));
        let formal = value.clone();
        shrink(&mut value);
        assert!(
            matches!(&value.layers[0].primitive, Primitive::Text { font_size, .. } if *font_size == expected)
        );
        assert!(
            matches!(&formal.layers[0].primitive, Primitive::Text { font_size, .. } if *font_size == font)
        );
        assert_eq!(value.duration_us, formal.duration_us);
    }
}
