use super::*;
fn reaction() -> BuildReaction {
    BuildReaction {
        center_x: 900.,
        center_y: 300.,
        width: 200.,
        height: 120.,
        start_scale: 0.5,
        fade: VisualFade {
            in_us: TimeUs(100_000),
            out_us: TimeUs(100_000),
        },
    }
}
#[test]
fn reaction_uses_measured_speech_and_preserves_ownership_through_save_and_undo() {
    let (p, mut r, speech) = recipe_with_bgm();
    let mut diagram = r.scenes[0].diagrams[0].clone();
    diagram.first_line = 1;
    diagram.last_line = 1;
    diagram.reaction = Some(reaction());
    r.scenes[0].diagrams.push(diagram);
    let stage = synthesize(&p, &r, &speech);
    let mut ids = layout_ids();
    ids.scenes[0].diagrams.push(id(599));
    let commands = plan_build_layout(&stage, &r, &speech, &ids).unwrap();
    let mut history = History::new(stage.clone()).unwrap();
    history.apply(0, &commands).unwrap();
    let full = history.project();
    let speech_clip = full
        .clips
        .iter()
        .find(|c| c.id == speech.speech_ids[1])
        .unwrap();
    let clip = full.clips.iter().find(|c| c.id == id(599)).unwrap();
    assert_eq!(clip.start_us, speech_clip.start_us);
    assert_eq!(clip.duration_us, speech_clip.duration_us);
    assert_eq!(clip.scene_id, speech_clip.scene_id);
    assert_eq!(
        (clip.transform.x, clip.transform.y, clip.transform.width),
        (850., 270., 100.)
    );
    assert_eq!(clip.transform.motion.as_ref().unwrap().end_scale, 2.);
    assert_eq!(clip.transform.fade.as_ref(), Some(&reaction().fade));
    let reopened: Project = serde_json::from_slice(&serde_json::to_vec(full).unwrap()).unwrap();
    assert_eq!(&reopened, full);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &stage);
}
#[test]
fn reaction_rejects_invalid_geometry_and_fades_longer_than_measured_speech() {
    let (p, mut r, speech) = recipe_with_bgm();
    let mut effect = reaction();
    effect.start_scale = 0.;
    r.scenes[0].diagrams[0].reaction = Some(effect);
    assert!(plan_build_script(&p, &r, &speech).is_err());
    let mut effect = reaction();
    effect.fade.in_us = TimeUs(20_000_000);
    r.scenes[0].diagrams[0].reaction = Some(effect);
    let stage = synthesize(&p, &r, &speech);
    assert!(plan_build_layout(&stage, &r, &speech, &layout_ids()).is_err());
}
