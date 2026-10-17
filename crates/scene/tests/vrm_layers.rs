use sungeki_project::*;
use sungeki_scene::{Primitive, ScenePlan};
fn fixture() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap()
}
#[test]
fn vrm_source_clock_handles_offsets_and_rational_frame_rates() {
    for (numerator, denominator) in [(24, 1), (30, 1), (60, 1), (30000, 1001)] {
        let mut p = fixture();
        p.canvas.fps = FrameRate {
            numerator,
            denominator,
        };
        p.clips[0].start_us = TimeUs(12345);
        p.clips[0].duration_us = TimeUs(900000);
        let before = p.clone();
        let plan = ScenePlan::new(&p).unwrap();
        assert!(!plan.layers.is_empty());
        for l in &plan.layers {
            assert!(l.start_us >= p.clips[0].start_us && l.end_us <= TimeUs(912345));
            assert_eq!(l.scene_id, p.clips[0].scene_id);
            let Primitive::Vrm {
                source_time_us,
                expressions,
                ..
            } = &l.primitive
            else {
                panic!()
            };
            assert_eq!(source_time_us.0, 500000 + l.start_us.0 - 12345);
            assert_eq!(expressions["aa"], 0.5);
        }
        assert_eq!(p, before);
    }
}
fn speech_fixture() -> Project {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut v = fixture();
    for (n, asset) in v.assets.iter_mut().enumerate() {
        asset.id = format!("00000000-0000-4000-8000-{:012}", n + 80);
    }
    let ClipContent::Vrm { standing } = &mut v.clips[0].content else {
        panic!()
    };
    standing.model_asset_id = v.assets[0].id.clone();
    standing.motion_asset_id = v.assets[1].id.clone();
    standing.character_id = Some(p.characters[0].id.clone());
    p.assets.extend(v.assets);
    p.clips[0].content = v.clips[0].content.clone();
    p
}
#[test]
fn speech_mouth_is_half_open_muted_and_missing_analysis_is_explicit() {
    let mut p = speech_fixture();
    let plan = ScenePlan::new(&p).unwrap();
    for layer in plan.layers.iter().filter(|l| l.clip_id == p.clips[0].id) {
        let Primitive::Vrm { expressions, .. } = &layer.primitive else {
            panic!()
        };
        assert_eq!(
            expressions["aa"],
            if (100000..300000).contains(&layer.start_us.0) {
                1.0
            } else {
                0.0
            }
        );
    }
    p.tracks[1].muted = true;
    assert!(ScenePlan::new(&p).unwrap().layers.iter().all(
        |l| matches!(&l.primitive, Primitive::Vrm { expressions, .. } if expressions["aa"] == 0.0)
    ));
    p.tracks[1].muted = false;
    let ClipContent::Speech { audio, .. } = &mut p.clips[1].content else {
        panic!()
    };
    *audio = None;
    assert!(
        ScenePlan::new(&p)
            .unwrap_err()
            .to_string()
            .contains("口パク解析")
    );
}
#[test]
fn unsupported_mouth_shape_and_layer_budget_reject() {
    let mut p = speech_fixture();
    let model_id = match &p.clips[0].content {
        ClipContent::Vrm { standing } => standing.model_asset_id.clone(),
        _ => panic!(),
    };
    let AssetProbe::Vrm(probe) = &mut p
        .assets
        .iter_mut()
        .find(|a| a.id == model_id)
        .unwrap()
        .probe
    else {
        panic!()
    };
    probe.inspection.expressions.clear();
    if let ClipContent::Vrm { standing } = &mut p.clips[0].content {
        standing.expressions.clear();
    }
    assert!(ScenePlan::new(&p).unwrap_err().to_string().contains("aa"));
    let mut p = fixture();
    p.clips[0].duration_us = TimeUs(4_000_000_000);
    p.scenes[0].duration_us = p.clips[0].duration_us;
    assert!(
        ScenePlan::new(&p)
            .unwrap_err()
            .to_string()
            .contains("100,000")
    );
}
#[test]
fn vrm_fade_uses_clip_clock_and_hidden_tracks_do_not_evaluate() {
    let mut p = fixture();
    p.clips[0].transform.fade = Some(VisualFade {
        in_us: TimeUs(200000),
        out_us: TimeUs(0),
    });
    p.clips[0].transform.flip_x = Some(true);
    let plan = ScenePlan::new(&p).unwrap();
    for layer in &plan.layers {
        assert_eq!(layer.transform.flip_x, Some(true));
        let expected = (layer.start_us.0 as f64 / 200000.0).min(1.0);
        assert!((layer.transform.opacity - expected).abs() < 0.00001);
    }
    p.tracks[0].visible = false;
    assert!(ScenePlan::new(&p).unwrap().layers.is_empty());
}
