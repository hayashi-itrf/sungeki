use sungeki_project::*;
use sungeki_scene::*;

fn fixture() -> (Project, serde_json::Value) {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    (
        serde_json::from_value(value["project"].clone()).unwrap(),
        value,
    )
}

#[test]
fn shared_boundaries_and_z_order_match_without_mutating_input() {
    let (project, fixture) = fixture();
    let before = project.clone();
    let plan = ScenePlan::new(&project).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let ids: Vec<_> = plan
            .layers_at(TimeUs(case["time_us"].as_u64().unwrap()))
            .map(|l| &l.clip_id)
            .collect();
        assert_eq!(serde_json::to_value(ids).unwrap(), case["clip_ids"]);
    }
    assert_eq!(project, before);
    assert_eq!(plan.frame_count, 7);
    assert!(plan.frame(7).is_none());
    assert_eq!(plan.audio.len(), 1);
    assert_eq!(plan.audio[0].clip_id, project.clips[4].id);
    assert!(plan.audio[0].source.asset_id().is_none()); // Unrenderable audio must remain visible to preflight.
    let subtitle = plan.layers.last().unwrap();
    assert_eq!(subtitle.transform, project.clips[5].transform);
    assert_eq!(
        subtitle.primitive,
        Primitive::Text {
            layout: None,
            outline: None,
            text: "visible subtitle".into(),
            font_size: 54.0,
            color: "#205b30".into(),
        }
    );
}

#[test]
fn spans_cover_every_frame_with_the_same_order_as_random_access() {
    let (project, _) = fixture();
    let plan = ScenePlan::new(&project).unwrap();
    let spans: Vec<_> = plan.visual_spans().collect();
    assert_eq!(spans.first().unwrap().frames.first, 0);
    assert_eq!(spans.last().unwrap().frames.end, plan.frame_count);
    for pair in spans.windows(2) {
        assert_eq!(pair[0].frames.end, pair[1].frames.first);
    }
    for span in &spans {
        for frame in span.frames.first..span.frames.end {
            let expected: Vec<_> = span
                .layer_indices
                .iter()
                .map(|i| &plan.layers[*i])
                .collect();
            assert_eq!(plan.frame(frame).unwrap(), expected);
        }
    }
    // A one-microsecond clip between frame times is not expanded to a full frame.
    assert!(!spans.iter().any(|s| {
        s.layer_indices
            .iter()
            .any(|i| plan.layers[*i].clip_id == project.clips[3].id)
    }));
}

#[test]
fn fractional_frame_rates_do_not_accumulate_rounding_error() {
    for fps in [
        FrameRate {
            numerator: 30,
            denominator: 1,
        },
        FrameRate {
            numerator: 30000,
            denominator: 1001,
        },
        FrameRate {
            numerator: u32::MAX,
            denominator: 1,
        },
    ] {
        for time in [0, 1, 33333, 33334, 100100, 123456789, MAX_TIME_US] {
            let frame = first_frame_at_or_after(&fps, TimeUs(time)).unwrap();
            if let Some(at) = fps.frame_time(frame) {
                assert!(at.0 >= time);
            }
            if frame > 0 {
                assert!(fps.frame_time(frame - 1).unwrap().0 < time);
            }
        }
    }
    let fps = FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    assert_eq!(fps.frame_time(30000), Some(TimeUs(1_001_000_000)));
    assert_eq!(first_frame_at_or_after(&fps, TimeUs(100100)), Some(3));
    assert_eq!(first_frame_at_or_after(&fps, TimeUs(100101)), Some(4));
    assert_eq!(first_frame_at_or_after(&fps, TimeUs(MAX_TIME_US + 1)), None);
}

#[test]
fn empty_scene_gaps_and_hidden_clips_keep_the_document_duration() {
    let (mut project, _) = fixture();
    project.clips.clear();
    let empty = ScenePlan::new(&project).unwrap();
    assert_eq!(empty.frame_count, 0);
    assert!(empty.visual_spans().next().is_none());
    project.scenes.push(Scene {
        fade: None,
        id: "00000000-0000-4000-8000-000000000020".into(),
        name: "empty scene".into(),
        start_us: TimeUs(5_000_000),
        duration_us: TimeUs(1_000_000),
    });
    let scene = ScenePlan::new(&project).unwrap();
    assert_eq!(scene.frame_count, 180);
    assert_eq!(
        scene.visual_spans().collect::<Vec<_>>(),
        vec![VisualSpan {
            frames: FrameSpan { first: 0, end: 180 },
            layer_indices: vec![]
        }]
    );
}

#[test]
fn invalid_document_cannot_enter_the_plan() {
    let (mut project, _) = fixture();
    project.canvas.fps.numerator = 0;
    assert!(ScenePlan::new(&project).is_err());
    project.canvas.fps.numerator = 30;
    project.clips[0].track_id = "missing".into();
    assert!(ScenePlan::new(&project).is_err());
}

#[test]
fn subtitle_outline_reaches_formal_scene_plan() {
    let (mut project, _) = fixture();
    let outline = sungeki_project::SubtitleOutline {
        width: 3,
        color: "#000000".into(),
    };
    project.characters[0].subtitle_style.outline = Some(outline.clone());
    project.characters[0].subtitle_style.layout = Some(sungeki_project::SubtitleLayout {
        alignment: sungeki_project::TextAlignment::Center,
        wrap: true,
    });
    let plan = ScenePlan::new(&project).unwrap();
    assert!(plan.layers.iter().any(|layer| matches!(&layer.primitive,
        Primitive::Text { outline: Some(value), layout: Some(layout), .. } if value == &outline && layout.wrap && layout.alignment == sungeki_project::TextAlignment::Center)));
}

#[test]
fn text_layout_survives_project_save_and_reaches_render_primitive() {
    let (mut project, _) = fixture();
    let clip = project
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Text { .. }))
        .unwrap();
    let id = clip.id.clone();
    let ClipContent::Text { layout, text, .. } = &mut clip.content else {
        panic!()
    };
    *layout = Some(SubtitleLayout {
        alignment: TextAlignment::Left,
        wrap: true,
    });
    *text = "Irodori一括制作・模擬音声・irodori-build-fixture-20261010-02".into();
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    let plan = ScenePlan::new(&restored).unwrap();
    let layer = plan.layers.iter().find(|l| l.clip_id == id).unwrap();
    let Primitive::Text { layout, text, .. } = &layer.primitive else {
        panic!()
    };
    assert_eq!(
        layout,
        &Some(SubtitleLayout {
            alignment: TextAlignment::Left,
            wrap: true
        })
    );
    assert_eq!(
        text,
        "Irodori一括制作・模擬音声・irodori-build-fixture-20261010-02"
    );
}

#[test]
fn free_text_outline_reaches_the_formal_scene_without_changing_geometry() {
    let (mut project, _) = fixture();
    let clip = project
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Text { .. }))
        .unwrap();
    let id = clip.id.clone();
    let transform = clip.transform.clone();
    let style = SubtitleOutline {
        width: 4,
        color: "#123456".into(),
    };
    let ClipContent::Text { outline, .. } = &mut clip.content else {
        panic!()
    };
    *outline = Some(style.clone());
    let plan = ScenePlan::new(&project).unwrap();
    let layer = plan.layers.iter().find(|l| l.clip_id == id).unwrap();
    assert_eq!(layer.transform, transform);
    assert!(
        matches!(&layer.primitive, Primitive::Text { outline: Some(value), .. } if value == &style)
    );
}
