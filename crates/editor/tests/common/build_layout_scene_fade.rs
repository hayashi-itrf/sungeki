use super::*;
#[test]
fn scene_fade_uses_actual_duration_and_survives_layout_save_and_undo() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.scenes[0].fade = Some(VisualFade {
        in_us: TimeUs(2_000_000),
        out_us: TimeUs(1_000_000),
    });
    let stage = synthesize(&p, &r, &speech);
    assert!(stage.scenes.iter().all(|s| s.fade.is_none()));
    let mut history = History::new(stage.clone()).unwrap();
    history
        .apply(
            0,
            &plan_build_layout(&stage, &r, &speech, &layout_ids()).unwrap(),
        )
        .unwrap();
    let full = history.project().clone();
    let scene = full
        .scenes
        .iter()
        .find(|s| s.id == speech.scene_ids[0])
        .unwrap();
    assert_eq!(scene.fade, r.scenes[0].fade);
    assert_eq!(scene.duration_us, stage.scenes[0].duration_us);
    for old in &stage.clips {
        assert_eq!(
            full.clips.iter().find(|c| c.id == old.id).unwrap().content,
            old.content
        );
    }
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&full).unwrap()).unwrap();
    assert_eq!(saved, full);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &stage);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &full);
}
#[test]
fn too_long_scene_fade_is_rejected_after_synthesis_without_mutation() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.scenes[0].fade = Some(VisualFade {
        in_us: TimeUs(60_000_000),
        out_us: TimeUs(0),
    });
    assert!(r.validate().is_ok());
    let stage = synthesize(&p, &r, &speech);
    let original = stage.clone();
    let error = plan_build_layout(&stage, &r, &speech, &layout_ids()).unwrap_err();
    assert!(error.0.contains("合成後"));
    assert_eq!(stage, original);
    r.scenes[0].fade = Some(VisualFade {
        in_us: TimeUs(u64::MAX),
        out_us: TimeUs(1),
    });
    assert!(r.validate().is_err());
}
