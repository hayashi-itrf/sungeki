use sungeki_project::*;
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p = Project::empty(id(1), "日本語の編集検証".into());
    p.tracks.push(Track {
        id: id(2),
        name: "文字".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    p.scenes.push(Scene {
        fade: None,
        id: id(3),
        name: "導入".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(2_000_000),
    });
    p.clips.push(Clip {
        id: id(4),
        track_id: id(2),
        scene_id: Some(id(3)),
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: Transform {
            motion: None,
            x: 10.,
            y: 20.,
            width: 400.,
            height: 100.,
            opacity: 1.,
            fade: None,
            flip_x: None,
        },
        content: ClipContent::Text {
            outline: None,
            layout: None,
            text: "こんにちは\n二行目".into(),
            font_size: 32.,
            color: "#112233".into(),
        },
    });
    p.groups.push(Group {
        id: id(5),
        name: "導入字幕".into(),
        clip_ids: vec![id(4)],
    });
    p
}
#[test]
fn json_roundtrip_retains_scene_and_group_membership() {
    let p = fixture();
    validate(&p).unwrap();
    let json = serde_json::to_string(&p).unwrap();
    let restored: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, p);
    validate(&restored).unwrap();
    assert!(json.contains("\"start_us\":0"));
    assert_eq!(restored.duration(), Some(TimeUs(2_000_000)));
}
#[test]
fn rejects_corrupt_structure_and_numeric_values() {
    let cases: Vec<fn(&mut Project)> = vec![
        |p| p.clips[0].id = p.tracks[0].id.clone(),
        |p| p.id = "not-uuid".into(),
        |p| p.clips[0].track_id = id(99),
        |p| p.clips[0].scene_id = Some(id(99)),
        |p| p.clips[0].start_us = TimeUs(2_000_000),
        |p| p.clips[0].duration_us = TimeUs(0),
        |p| p.clips[0].start_us = TimeUs(u64::MAX),
        |p| p.clips[0].transform.x = f64::NAN,
        |p| p.clips[0].transform.opacity = 1.1,
        |p| p.clips[0].transform.width = 0.,
        |p| p.canvas.fps.denominator = 0,
        |p| p.canvas.width = 0,
        |p| p.canvas.background = "#日本語".into(),
        |p| p.groups[0].clip_ids.push(id(4)),
        |p| p.groups[0].clip_ids.clear(),
        |p| p.groups[0].clip_ids[0] = id(99),
        |p| {
            p.scenes.push(Scene {
                fade: None,
                id: id(6),
                name: "衝突".into(),
                start_us: TimeUs(1),
                duration_us: TimeUs(1),
            })
        },
        |p| {
            p.audio_mix.normalization = Some(Normalization {
                target_lufs: f64::INFINITY,
                true_peak_db: -1.,
            })
        },
    ];
    for (index, mutate) in cases.iter().enumerate() {
        let mut p = fixture();
        mutate(&mut p);
        assert!(validate(&p).is_err(), "case {index}");
    }
}
#[test]
fn groups_cannot_cross_scene_boundaries() {
    let mut p = fixture();
    let mut other = p.clips[0].clone();
    other.id = id(7);
    other.scene_id = None;
    p.clips.push(other);
    p.groups[0].clip_ids.push(id(7));
    assert!(validate(&p).is_err());
}
#[test]
fn adjacent_scenes_are_valid_and_schema_is_strict() {
    let mut p = fixture();
    p.scenes.push(Scene {
        fade: None,
        id: id(6),
        name: "次".into(),
        start_us: TimeUs(2_000_000),
        duration_us: TimeUs(1),
    });
    validate(&p).unwrap();
    let mut json = serde_json::to_value(&p).unwrap();
    json["extra"] = true.into();
    assert!(serde_json::from_value::<Project>(json).is_err());
    p.schema_version = 99;
    assert!(validate(&p).is_err());
}
#[test]
fn rational_frame_time_has_no_accumulated_rounding() {
    let fps = FrameRate {
        numerator: 30_000,
        denominator: 1001,
    };
    assert_eq!(fps.frame_time(30_000), Some(TimeUs(1_001_000_000)));
    assert_eq!(fps.frame_time(u64::MAX), None);
    assert!(serde_json::from_str::<TimeUs>("-1").is_err());
    assert!(serde_json::from_str::<TimeUs>("1.5").is_err());
}
#[test]
fn empty_document_is_valid() {
    let p = Project::empty(id(1), "空".into());
    validate(&p).unwrap();
    assert_eq!(p.duration(), Some(TimeUs(0)));
}
