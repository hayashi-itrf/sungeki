use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
fn fixture() -> Project {
    apply_batch(&Project::empty(id(1), "音声実尺".into()), &commands()).unwrap()
}
#[test]
fn duration_and_scene_extension_on_separate_tracks_are_one_undo_without_ripple() {
    let mut p = fixture();
    let mut next = p.clips[0].clone();
    next.id = id(6);
    next.start_us = TimeUs(4_000_000);
    next.duration_us = TimeUs(1_000_000);
    p.clips.push(next.clone());
    assert!(plan_speech_timing(&p, &id(4), TimeUs(6_000_000)).is_err());
    let mut track = p.tracks[0].clone();
    track.id = id(8);
    p.tracks.push(track);
    next.track_id = id(8);
    p.clips[1] = next.clone();
    let plan = plan_speech_timing(&p, &id(4), TimeUs(6_000_000)).unwrap();
    assert_eq!(plan.overlapping_clip_ids, vec![id(6)]);
    let mut history = History::new(p.clone()).unwrap();
    history.apply(0, &plan.commands).unwrap();
    assert_eq!(history.project().clips[0].duration_us, TimeUs(6_000_000));
    assert_eq!(history.project().scenes[0].duration_us, TimeUs(6_000_000));
    assert_eq!(history.project().clips[1], next);
    assert_eq!(history.project().clips[0].scene_id, Some(id(5)));
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
}
#[test]
fn scene_collision_is_structured_and_boundary_contact_is_allowed() {
    let mut p = fixture();
    p.scenes.push(Scene {
        fade: None,
        id: id(7),
        name: "次".into(),
        start_us: TimeUs(6_000_000),
        duration_us: TimeUs(2_000_000),
    });
    assert!(plan_speech_timing(&p, &id(4), TimeUs(6_000_000)).is_ok());
    assert_eq!(
        plan_speech_timing(&p, &id(4), TimeUs(6_000_001)),
        Err(SpeechTimingError::SceneConflict {
            scene_id: id(5),
            required_end_us: TimeUs(6_000_001),
            conflicting_scene_ids: vec![id(7)]
        })
    );
    assert_eq!(p.clips[0].duration_us, TimeUs(2_000_000));
    assert_eq!(p.scenes[0].duration_us, TimeUs(5_000_000));
}
#[test]
fn shortening_keeps_scene_and_touching_speech_is_not_overlap() {
    let mut p = fixture();
    let mut next = p.clips[0].clone();
    next.id = id(6);
    next.start_us = TimeUs(1_000_000);
    p.clips.push(next);
    let plan = plan_speech_timing(&p, &id(4), TimeUs(1_000_000)).unwrap();
    assert!(plan.overlapping_clip_ids.is_empty());
    let result = apply_batch(&p, &plan.commands).unwrap();
    assert_eq!(result.scenes, p.scenes);
    assert_eq!(result.clips[1], p.clips[1]);
}
#[test]
fn independent_clip_overlaps_other_tracks_and_rejects_invalid_duration() {
    let mut p = fixture();
    p.clips[0].scene_id = None;
    p.clips[0].start_us = TimeUs(100);
    let mut track = p.tracks[0].clone();
    track.id = id(8);
    track.muted = true;
    p.tracks.push(track);
    let mut other = p.clips[0].clone();
    other.id = id(9);
    other.track_id = id(8);
    p.clips.push(other);
    let plan = plan_speech_timing(&p, &id(4), TimeUs(50)).unwrap();
    assert_eq!(plan.overlapping_clip_ids, vec![id(9)]);
    assert_eq!(apply_batch(&p, &plan.commands).unwrap().scenes, p.scenes);
    for duration in [0, MAX_TIME_US, u64::MAX] {
        assert!(matches!(
            plan_speech_timing(&p, &id(4), TimeUs(duration)),
            Err(SpeechTimingError::Invalid(_))
        ));
    }
    assert!(plan_speech_timing(&p, &id(99), TimeUs(10)).is_err());
}
#[test]
fn extension_uses_scene_origin_and_preserves_other_members() {
    let mut p = fixture();
    p.scenes[0].start_us = TimeUs(10_000_000);
    p.clips[0].start_us = TimeUs(12_000_000);
    let mut sibling = p.clips[0].clone();
    sibling.id = id(6);
    sibling.start_us = TimeUs(10_000_000);
    p.clips.push(sibling.clone());
    let plan = plan_speech_timing(&p, &id(4), TimeUs(6_000_000)).unwrap();
    let result = apply_batch(&p, &plan.commands).unwrap();
    assert_eq!(result.scenes[0].start_us, TimeUs(10_000_000));
    assert_eq!(result.scenes[0].duration_us, TimeUs(8_000_000));
    assert_eq!(result.clips[1], sibling);
    assert!(plan.overlapping_clip_ids.is_empty());
}
