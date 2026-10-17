use super::*;
fn motion() -> VisualMotion {
    VisualMotion {
        end_x: 480.,
        end_y: 180.,
        end_scale: 0.8,
        easing: MotionEasing::EaseInOut,
    }
}
#[test]
fn diagram_motion_spans_synthesized_speech_and_preserves_audio() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.scenes[0].diagrams[0].first_line = 0;
    r.scenes[0].diagrams[0].last_line = 1;
    r.scenes[0].diagrams[0].motion = Some(motion());
    r.scenes[0].diagrams[0].fade = Some(VisualFade {
        in_us: TimeUs(100000),
        out_us: TimeUs(200000),
    });
    let stage = synthesize(&p, &r, &speech);
    let ids = layout_ids();
    let full = apply_batch(
        &stage,
        &plan_build_layout(&stage, &r, &speech, &ids).unwrap(),
    )
    .unwrap();
    let diagram = full
        .clips
        .iter()
        .find(|c| c.id == ids.scenes[0].diagrams[0])
        .unwrap();
    let first = full
        .clips
        .iter()
        .find(|c| c.id == speech.speech_ids[0])
        .unwrap();
    let last = full
        .clips
        .iter()
        .find(|c| c.id == speech.speech_ids[1])
        .unwrap();
    assert_eq!(diagram.start_us, first.start_us);
    assert_eq!(
        diagram.duration_us.0,
        last.start_us.0 + last.duration_us.0 - first.start_us.0
    );
    assert_eq!(diagram.transform.motion, Some(motion()));
    assert_eq!(diagram.transform.fade, r.scenes[0].diagrams[0].fade);
    assert_eq!(first.content, stage.clips[0].content);
    assert_eq!(last.content, stage.clips[1].content);
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&full).unwrap()).unwrap();
    assert_eq!(restored, full);
}
#[test]
fn diagram_motion_rejects_conflicting_effects_and_oversized_end_geometry() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.scenes[0].diagrams[0].motion = Some(motion());
    r.scenes[0].diagrams[0].reaction = Some(BuildReaction {
        center_x: 400.,
        center_y: 300.,
        width: 200.,
        height: 200.,
        start_scale: 0.5,
        fade: VisualFade {
            in_us: TimeUs(0),
            out_us: TimeUs(0),
        },
    });
    assert!(r.validate().is_err());
    r.scenes[0].diagrams[0].reaction = None;
    let stage = synthesize(&p, &r, &speech);
    r.scenes[0].diagrams[0].motion.as_mut().unwrap().end_scale = 100000.;
    let error = plan_build_script(&p, &r, &speech).unwrap_err();
    assert!(error.0.contains("シーン1・説明図1"));
    assert!(plan_build_layout(&stage, &r, &speech, &layout_ids()).is_err());
    r.scenes[0].diagrams[0].motion.as_mut().unwrap().end_x = f64::NAN;
    assert!(r.validate().is_err());
}

#[test]
fn motion_preflight_uses_portrait_dimensions_without_requiring_synthesized_audio() {
    let (p, mut r, ids) = recipe_with_bgm();
    r.preset_id = "narration-portrait-v1".into();
    r.foreword = None;
    r.roles.truncate(1);
    for scene in &mut r.scenes {
        for line in &mut scene.lines {
            line.role = r.roles[0].role.clone();
        }
    }
    r.scenes[0].diagrams[0].motion = Some(motion());
    let commands = plan_build_script(&p, &r, &ids).unwrap();
    let stage = apply_batch(&p, &commands).unwrap();
    assert_eq!(stage.canvas.width, 1080);
    assert!(stage.clips.iter().all(|c| c.transform.motion.is_none()));
    r.scenes[0].diagrams[0].motion.as_mut().unwrap().end_scale = 100000.;
    assert!(plan_build_script(&p, &r, &ids).is_err());
    assert!(p.clips.is_empty());
}

#[test]
fn diagram_fade_rejects_overflow_conflict_and_excess_actual_duration() {
    let (p, mut r, speech) = recipe_with_bgm();
    r.scenes[0].diagrams[0].fade = Some(VisualFade {
        in_us: TimeUs(60_000_000),
        out_us: TimeUs(0),
    });
    assert!(r.validate().is_ok());
    let stage = synthesize(&p, &r, &speech);
    let original = stage.clone();
    assert!(
        plan_build_layout(&stage, &r, &speech, &layout_ids())
            .unwrap_err()
            .0
            .contains("合成後")
    );
    assert_eq!(stage, original);
    r.scenes[0].diagrams[0].fade.as_mut().unwrap().in_us = TimeUs(u64::MAX);
    assert!(r.validate().is_err());
    r.scenes[0].diagrams[0].fade.as_mut().unwrap().in_us = TimeUs(0);
    r.scenes[0].diagrams[0].reaction = Some(BuildReaction {
        center_x: 400.,
        center_y: 300.,
        width: 200.,
        height: 200.,
        start_scale: 0.5,
        fade: VisualFade {
            in_us: TimeUs(0),
            out_us: TimeUs(0),
        },
    });
    assert!(r.validate().is_err());
}
