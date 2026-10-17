use super::*;
#[test]
fn bgm_points_follow_final_bgm_length_with_foreword_and_save_undo() {
    let (p, mut r, speech) = recipe_with_bgm();
    let bgm = r.bgm.as_mut().unwrap();
    bgm.volume_point_reference_duration_us = Some(TimeUs(100_000_000));
    bgm.playback.volume_points = vec![
        VolumePoint {
            at_us: TimeUs(0),
            level: 0.2,
        },
        VolumePoint {
            at_us: TimeUs(50_000_000),
            level: 1.0,
        },
        VolumePoint {
            at_us: TimeUs(100_000_000),
            level: 0.3,
        },
    ];
    let original = bgm.playback.clone();
    let stage = synthesize(&p, &r, &speech);
    let mut history = History::new(stage.clone()).unwrap();
    history
        .apply(
            0,
            &plan_build_layout(&stage, &r, &speech, &layout_ids()).unwrap(),
        )
        .unwrap();
    let full = history.project().clone();
    let clip = full.clips.iter().find(|c| c.id == id(550)).unwrap();
    assert_eq!(clip.start_us, r.foreword.as_ref().unwrap().duration_us);
    let ClipContent::Audio { playback, .. } = &clip.content else {
        panic!()
    };
    assert_eq!(playback.volume_points[0].at_us, TimeUs(0));
    assert_eq!(
        playback.volume_points[1].at_us,
        TimeUs((clip.duration_us.0 + 1) / 2)
    );
    assert_eq!(playback.volume_points[2].at_us, clip.duration_us);
    assert_eq!(playback.fade_in_us, original.fade_in_us);
    assert_eq!(playback.fade_out_us, original.fade_out_us);
    assert_eq!(r.bgm.as_ref().unwrap().playback, original);
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&full).unwrap()).unwrap();
    assert_eq!(saved, full);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &stage);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &full);
    r.bgm.as_mut().unwrap().volume_point_reference_duration_us = None;
    assert!(plan_build_layout(&stage, &r, &speech, &layout_ids()).is_err());
}
#[test]
fn invalid_reference_and_collapsed_points_are_rejected() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.bgm.as_mut().unwrap().volume_point_reference_duration_us = Some(TimeUs(0));
    assert!(r.validate().is_err());
    r.bgm.as_mut().unwrap().volume_point_reference_duration_us = Some(TimeUs(MAX_TIME_US));
    r.bgm.as_mut().unwrap().playback.volume_points = vec![
        VolumePoint {
            at_us: TimeUs(0),
            level: 0.2,
        },
        VolumePoint {
            at_us: TimeUs(1),
            level: 1.0,
        },
    ];
    let stage = synthesize(&p, &r, &speech);
    let copy = stage.clone();
    assert!(
        plan_build_layout(&stage, &r, &speech, &layout_ids())
            .unwrap_err()
            .0
            .contains("重なり")
    );
    assert_eq!(stage, copy);
    r.bgm.as_mut().unwrap().volume_point_reference_duration_us = Some(TimeUs(1));
    r.bgm.as_mut().unwrap().playback.volume_points[1].at_us = TimeUs(2);
    assert!(r.validate().is_err());
}
