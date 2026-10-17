use sungeki_jobs::{BuildSchedule, BuildStep};

#[test]
fn all_speech_then_save_render_and_review_are_required() {
    let mut state = BuildSchedule::new(22).unwrap();
    let mut expected = vec![BuildStep::Prepare, BuildStep::Compose];
    expected.extend((0..22).map(BuildStep::Speech));
    expected.extend([
        BuildStep::Layout,
        BuildStep::Validate,
        BuildStep::Bake,
        BuildStep::Save,
        BuildStep::Render,
        BuildStep::Review,
    ]);
    for (i, step) in expected.into_iter().enumerate() {
        assert_eq!(state.step(), Some(step));
        let id = format!("child-{i}");
        state.reserve(&id).unwrap();
        state.reserve(&id).unwrap();
        let reserved = state.clone();
        assert!(state.reserve("different").is_err());
        assert!(state.acknowledge("different").is_err());
        assert_eq!(state, reserved);
        // Persisted fields reconstruct exactly, including an uncertain child.
        state = BuildSchedule::restore(state.speech_count(), state.completed(), Some(id.clone()))
            .unwrap();
        assert_eq!(state, reserved);
        state.acknowledge(&id).unwrap();
        assert!(state.acknowledge(&id).is_err());
    }
    assert_eq!(state.completed(), 30);
    assert_eq!(state.step(), None);
    assert!(state.reserve("late").is_err());
}

#[test]
fn corrupt_or_unbounded_persisted_cursors_are_rejected() {
    for count in [0, 201, u16::MAX] {
        assert!(BuildSchedule::new(count).is_err());
    }
    assert!(BuildSchedule::restore(1, 10, None).is_err());
    assert!(BuildSchedule::restore(1, 9, Some("child".into())).is_err());
    assert!(BuildSchedule::restore(1, 0, Some("bad/path".into())).is_err());
    assert!(BuildSchedule::restore(1, 0, Some("x".repeat(129))).is_err());
    let final_state = BuildSchedule::restore(200, 208, None).unwrap();
    assert!(final_state.step().is_none());
}
