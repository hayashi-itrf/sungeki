use sungeki_editor::*;
use sungeki_project::*;
fn project() -> Project {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    serde_json::from_value(value["project"].clone()).unwrap()
}
#[test]
fn blur_is_atomic_typed_and_persists_through_history() {
    let mut p = project();
    let clip = p
        .clips
        .iter_mut()
        .find(|c| matches!(c.content, ClipContent::Shape { .. }))
        .unwrap();
    let id = clip.id.clone();
    clip.content = ClipContent::Blur { sigma: 18. };
    let mut h = History::new(p.clone()).unwrap();
    for sigma in [-1., 129., f64::NAN, f64::INFINITY] {
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "must rollback".into()
                    },
                    Command::SetBlur {
                        clip_id: id.clone(),
                        sigma
                    }
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &p);
    }
    h.apply(
        0,
        &[Command::SetBlur {
            clip_id: id.clone(),
            sigma: 0.,
        }],
    )
    .unwrap();
    let next = h.project().clone();
    let decoded: Project = serde_json::from_str(&serde_json::to_string(&next).unwrap()).unwrap();
    assert_eq!(next, decoded);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
    let text = next
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Text { .. }))
        .unwrap();
    assert!(
        h.apply(
            3,
            &[Command::SetBlur {
                clip_id: text.id.clone(),
                sigma: 18.
            }]
        )
        .is_err()
    );
    assert_eq!(h.project(), &next);
}
