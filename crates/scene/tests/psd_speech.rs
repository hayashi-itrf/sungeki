use sungeki_project::*;
use sungeki_scene::*;
fn project() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap()
}
fn option(plan: &ScenePlan, us: u64) -> String {
    let layers: Vec<_> = plan.layers_at(TimeUs(us)).collect();
    assert_eq!(layers.len(), 1);
    let Primitive::Psd { pose, .. } = &layers[0].primitive else {
        panic!()
    };
    pose.selections[0].option.clone()
}
#[test]
fn half_open_mouth_boundaries_preserve_standing_and_original() {
    let p = project();
    let before = p.clone();
    let plan = ScenePlan::new(&p).unwrap();
    assert_eq!(plan.layers.len(), 3);
    for (at, open) in [
        (0, false),
        (99999, false),
        (100000, true),
        (299999, true),
        (300000, false),
        (999999, false),
    ] {
        assert_eq!(option(&plan, at), if open { "/0/0" } else { "/0/1" });
    }
    assert!(plan.layers_at(TimeUs(1_000_000)).next().is_none());
    for layer in &plan.layers {
        assert_eq!(layer.clip_id, p.clips[0].id);
        assert_eq!(layer.scene_id, p.clips[0].scene_id);
        assert_eq!(layer.transform, p.clips[0].transform);
    }
    assert_eq!(p, before);
}
#[test]
fn muted_speech_is_closed_and_missing_analysis_is_explicit() {
    let mut p = project();
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &mut p.clips[1].content
    else {
        panic!()
    };
    audio.lip_sync = None;
    assert!(ScenePlan::new(&p).unwrap_err().0.contains("口パク解析"));
    p.tracks[1].muted = true;
    assert_eq!(option(&ScenePlan::new(&p).unwrap(), 200000), "/0/1");
    p.tracks[1].muted = false;
    p.clips[1].start_us = TimeUs(1_000_000);
    assert_eq!(option(&ScenePlan::new(&p).unwrap(), 200000), "/0/1");
}
#[test]
fn overlapping_same_character_uses_union_and_keeps_z_order() {
    let mut p = project();
    let mut second = p.clips[1].clone();
    second.id = "00000000-0000-4000-8000-000000000010".into();
    second.start_us = TimeUs(100000);
    p.clips.push(second);
    let mut overlay = p.clips[0].clone();
    overlay.id = "00000000-0000-4000-8000-000000000011".into();
    overlay.content = ClipContent::Text {
        outline: None,
        layout: None,
        text: "front".into(),
        font_size: 20.,
        color: "#000000".into(),
    };
    p.clips.push(overlay.clone());
    let plan = ScenePlan::new(&p).unwrap();
    let layer = plan.layers_at(TimeUs(350000)).next().unwrap();
    let Primitive::Psd { pose, .. } = &layer.primitive else {
        panic!()
    };
    assert_eq!(pose.selections[0].option, "/0/0");
    assert_eq!(layer.end_us, TimeUs(400000));
    assert_eq!(
        plan.layers_at(TimeUs(350000)).last().unwrap().clip_id,
        overlay.id
    );
}
#[test]
fn fractional_fps_spans_match_every_frame_and_random_access() {
    let mut p = project();
    p.canvas.fps = FrameRate {
        numerator: 30000,
        denominator: 1001,
    };
    let plan = ScenePlan::new(&p).unwrap();
    let spans: Vec<_> = plan.visual_spans().collect();
    for index in (0..plan.frame_count).rev() {
        let span = spans
            .iter()
            .find(|s| s.frames.first <= index && index < s.frames.end)
            .unwrap();
        let actual = plan.frame(index).unwrap();
        assert_eq!(
            actual,
            span.layer_indices
                .iter()
                .map(|i| &plan.layers[*i])
                .collect::<Vec<_>>()
        );
        let us = p.canvas.fps.frame_time(index).unwrap().0;
        assert_eq!(
            option(&plan, us),
            if (100000..300000).contains(&us) {
                "/0/0"
            } else {
                "/0/1"
            }
        );
    }
}

#[test]
fn sample_boundaries_round_up_without_opening_early_or_truncating() {
    let mut p = project();
    let AssetProbe::Audio(probe) = &mut p.assets[1].probe else {
        panic!()
    };
    probe.sample_rate = 10001;
    probe.frames = 10001;
    probe.byte_length = 20046;
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &mut p.clips[1].content
    else {
        panic!()
    };
    let timing = audio.lip_sync.as_mut().unwrap();
    timing.sample_rate = 10001;
    timing.source_frames = 10001;
    timing.open = vec![MouthInterval {
        start_frame: 401,
        end_frame: 2001,
    }];
    let plan = ScenePlan::new(&p).unwrap();
    for (at, open) in [
        (40095, false),
        (40096, true),
        (200079, true),
        (200080, false),
    ] {
        assert_eq!(option(&plan, at), if open { "/0/0" } else { "/0/1" });
    }
}

#[test]
fn mouth_changes_do_not_restart_visual_fade() {
    let mut p = project();
    let standing = p
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Psd { .. }))
        .unwrap();
    standing.transform.flip_x = Some(true);
    standing.transform.fade = Some(VisualFade {
        in_us: standing.duration_us,
        out_us: TimeUs(0),
    });
    let id = standing.id.clone();
    let start = standing.start_us.0;
    let duration = standing.duration_us.0;
    let plan = ScenePlan::new(&p).unwrap();
    for index in 0..plan.frame_count {
        let time = p.canvas.fps.frame_time(index).unwrap().0;
        if let Some(layer) = plan
            .frame(index)
            .unwrap()
            .into_iter()
            .find(|l| l.clip_id == id)
        {
            assert_eq!(layer.transform.flip_x, Some(true));
            assert!(
                (layer.transform.opacity - (time - start) as f64 / duration as f64).abs() < 1e-10
            );
        }
    }
}
