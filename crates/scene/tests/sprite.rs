use sungeki_project::*;
use sungeki_scene::*;

fn fixture() -> (Project, SpriteSet) {
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let mut ids = vec![];
    for n in 20..22 {
        let id = format!("00000000-0000-4000-8000-{n:012}");
        p.assets.push(Asset {
            id: id.clone(),
            name: format!("mouth{n}"),
            locator: AssetLocator::Absolute {
                path: format!("/fixture/mouth{n}.png"),
            },
            source: None,
            credit: "fixture".into(),
            content_hash: Some("a".repeat(64)),
            probe: AssetProbe::Image(ImageProbe {
                format: ImageFormat::Png,
                width: 320,
                height: 640,
                orientation: 1,
                has_alpha: true,
                byte_length: 100,
                sha256: "a".repeat(64),
            }),
        });
        ids.push(id);
    }
    let set = SpriteSet {
        default_pose: "通常".into(),
        poses: vec![SpritePose {
            name: "通常".into(),
            closed_asset_id: ids[0].clone(),
            open_asset_id: Some(ids[1].clone()),
        }],
    };
    (p, set)
}
fn evaluate(p: &Project, set: &SpriteSet) -> Result<Vec<Layer>, ValidationError> {
    sprite_layers(p, &p.clips[0].id, set, None, Some(&p.characters[0].id))
}
#[test]
fn sprite_boundaries_match_psd_and_preserve_layout() {
    let (p, set) = fixture();
    let before = p.clone();
    let layers = evaluate(&p, &set).unwrap();
    let psd = ScenePlan::new(&p).unwrap();
    assert_eq!(layers.len(), 3);
    for (sprite, psd) in layers.iter().zip(&psd.layers) {
        assert_eq!(sprite.start_us, psd.start_us);
        assert_eq!(sprite.end_us, psd.end_us);
        assert_eq!(sprite.transform, psd.transform);
        assert_eq!(sprite.clip_id, psd.clip_id);
        assert_eq!(sprite.scene_id, psd.scene_id);
    }
    for us in [0, 99_999, 100_000, 299_999, 300_000, 999_999] {
        let layer = layers
            .iter()
            .find(|l| l.start_us.0 <= us && us < l.end_us.0)
            .unwrap();
        let Primitive::Image { asset_id } = &layer.primitive else {
            panic!()
        };
        assert_eq!(
            asset_id,
            set.asset_id(None, (100_000..300_000).contains(&us))
                .unwrap()
        );
    }
    assert_eq!(p, before);
}
#[test]
fn muted_hidden_static_and_missing_analysis_are_distinct() {
    let (mut p, mut set) = fixture();
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &mut p.clips[1].content
    else {
        panic!()
    };
    audio.lip_sync = None;
    assert!(evaluate(&p, &set).unwrap_err().0.contains("口パク解析"));
    p.tracks[1].muted = true;
    assert_eq!(evaluate(&p, &set).unwrap().len(), 1);
    p.tracks[1].muted = false;
    set.poses[0].open_asset_id = None;
    assert_eq!(evaluate(&p, &set).unwrap().len(), 1);
    p.tracks[0].visible = false;
    assert!(evaluate(&p, &set).unwrap().is_empty());
    assert!(sprite_layers(&p, &p.clips[0].id, &set, Some("unknown"), None).is_err());
    assert!(sprite_layers(&p, &p.clips[0].id, &set, None, Some("unknown")).is_err());
}
#[test]
fn overlapping_utterances_merge_and_standing_crops_open_interval() {
    let (mut p, set) = fixture();
    let mut speech = p.clips[1].clone();
    speech.id = "00000000-0000-4000-8000-000000000030".into();
    speech.start_us = TimeUs(100_000);
    p.clips.push(speech);
    p.clips[0].start_us = TimeUs(150_000);
    p.clips[0].duration_us = TimeUs(200_000);
    let layers = evaluate(&p, &set).unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].start_us, TimeUs(150_000));
    assert_eq!(layers[0].end_us, TimeUs(350_000));
    assert_eq!(
        layers[0].primitive,
        Primitive::Image {
            asset_id: set.poses[0].open_asset_id.clone().unwrap(),
        }
    );
}

#[test]
fn saved_sprite_flows_through_scene_plan_and_parent_fade_clock() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/sprite-project.json")).unwrap();
    p.clips[0].transform.fade = Some(VisualFade {
        in_us: TimeUs(1_000_000),
        out_us: TimeUs(0),
    });
    p.clips[0].transform.flip_x = Some(true);
    let plan = ScenePlan::new(&p).unwrap();
    assert_eq!(plan.layers.len() as u64, plan.frame_count);
    for frame in 0..plan.frame_count {
        let time = p.canvas.fps.frame_time(frame).unwrap().0;
        let evaluated = plan.frame(frame).unwrap();
        let layer = evaluated
            .iter()
            .find(|l| l.clip_id == p.clips[0].id)
            .unwrap();
        assert!((layer.transform.opacity - time as f64 / 1_000_000.).abs() < 1e-10);
        assert_eq!(layer.transform.flip_x, Some(true));
        let Primitive::Image { asset_id } = &layer.primitive else {
            panic!()
        };
        let ClipContent::Sprite { sprites, .. } = &p.clips[0].content else {
            panic!()
        };
        assert_eq!(
            asset_id,
            sprites
                .asset_id(None, (100_000..300_000).contains(&time))
                .unwrap()
        );
    }
}

#[test]
fn shared_preview_sample_boundaries_match_formal_scene_plan() {
    #[derive(serde::Deserialize)]
    struct Case {
        sample_rate: u32,
        start_frame: u64,
        end_frame: u64,
        queries: Vec<(u64, bool)>,
    }
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/sprite-boundaries.json"
    ))
    .unwrap();
    for case in cases {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/sprite-project.json"))
                .unwrap();
        let AssetProbe::Audio(probe) = &mut p.assets[1].probe else {
            panic!()
        };
        probe.sample_rate = case.sample_rate;
        probe.frames = u64::from(case.sample_rate);
        probe.byte_length = 44 + probe.frames * 2;
        let ClipContent::Speech {
            audio: Some(audio), ..
        } = &mut p.clips[1].content
        else {
            panic!()
        };
        let timing = audio.lip_sync.as_mut().unwrap();
        timing.sample_rate = case.sample_rate;
        timing.source_frames = u64::from(case.sample_rate);
        timing.open = vec![MouthInterval {
            start_frame: case.start_frame,
            end_frame: case.end_frame,
        }];
        let plan = ScenePlan::new(&p).unwrap();
        let ClipContent::Sprite { sprites, .. } = &p.clips[0].content else {
            panic!()
        };
        for (time, open) in case.queries.into_iter().rev() {
            let layer = plan.layers_at(TimeUs(time)).next().unwrap();
            assert_eq!(
                layer.primitive,
                Primitive::Image {
                    asset_id: sprites.asset_id(None, open).unwrap().into()
                }
            );
        }
    }
}
