use sungeki_editor::*;
use sungeki_project::*;
#[test]
fn scene_fade_is_atomic_serialized_and_undoable() {
    let mut p = Project::empty(
        "00000000-0000-4000-8000-000000000001".into(),
        "全体fade".into(),
    );
    let id = "00000000-0000-4000-8000-000000000002".to_string();
    p.scenes.push(Scene {
        id: id.clone(),
        name: "導入".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(3_000_000),
        fade: None,
    });
    let fade = Some(VisualFade {
        in_us: TimeUs(1_000_000),
        out_us: TimeUs(1_000_000),
    });
    let command = Command::SetSceneFade {
        scene_id: id.clone(),
        fade: fade.clone(),
    };
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[command.clone()]).unwrap();
    let changed = h.project().clone();
    assert_eq!(changed.scenes[0].fade, fade);
    assert_eq!(
        serde_json::from_str::<Project>(&serde_json::to_string(&changed).unwrap()).unwrap(),
        changed
    );
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &changed);
    assert!(
        apply_batch(
            &p,
            &[
                command,
                Command::ResizeScene {
                    scene_id: id.clone(),
                    duration_us: TimeUs(1)
                }
            ]
        )
        .is_err()
    );
    assert!(
        apply_batch(
            &p,
            &[Command::SetSceneFade {
                scene_id: "missing".into(),
                fade: None
            }]
        )
        .is_err()
    );
    assert_eq!(
        apply_batch(
            &changed,
            &[Command::SetSceneFade {
                scene_id: id,
                fade: None
            }]
        )
        .unwrap(),
        p
    );
}
