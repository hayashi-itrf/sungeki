use sungeki_editor::*;
use sungeki_project::*;
#[test]
fn motion_preserves_content_and_is_atomic_undoable_and_saved() {
    let p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    let clip = &p.clips[0];
    let mut t = clip.transform.clone();
    t.motion = Some(VisualMotion {
        end_x: 100.,
        end_y: 100.,
        end_scale: 1.2,
        easing: MotionEasing::EaseInOut,
    });
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[Command::SetTransform {
            clip_id: clip.id.clone(),
            transform: t.clone(),
        }],
    )
    .unwrap();
    let next = h.project().clone();
    assert_eq!(next.clips[0].content, clip.content);
    assert_eq!(
        serde_json::from_str::<Project>(&serde_json::to_string(&next).unwrap()).unwrap(),
        next
    );
    t.motion.as_mut().unwrap().end_scale = 0.;
    assert!(
        h.apply(
            1,
            &[Command::SetTransform {
                clip_id: clip.id.clone(),
                transform: t
            }]
        )
        .is_err()
    );
    assert_eq!(h.project(), &next);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
    let mut wrong = next.clone();
    wrong.clips[0].content = ClipContent::Text {
        outline: None,
        layout: None,
        text: "text".into(),
        font_size: 24.,
        color: "#ffffff".into(),
    };
    assert!(validate(&wrong).is_err());
}
