use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p = Project::empty(id(1), "Ripple".into());
    p.tracks.push(Track {
        id: id(2),
        name: "Visual".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    for (n, start) in [(3, 0), (4, 2_000_000), (5, 4_000_000)] {
        p.clips.push(serde_json::from_value(serde_json::json!({"id":id(n),"track_id":id(2),"scene_id":null,
            "start_us":start,"duration_us":1_000_000,"transform":{"x":0,"y":0,"width":100,"height":100,"opacity":1},
            "content":{"type":"shape","shape":"rectangle","fill":"#000000"}})).unwrap());
    }
    p
}
fn ripple(ids: Vec<String>, boundary: u64, delta: i64) -> Command {
    Command::Ripple {
        clip_ids: ids,
        boundary_us: TimeUs(boundary),
        delta_us: delta,
    }
}
#[test]
fn selected_boundary_moves_once_preserves_content_and_undo_replay() {
    let p = fixture();
    let mut session = EditSession::new(p.clone()).unwrap();
    let request = EditRequest {
        operation_id: id(10),
        expected_revision: 0,
        action: EditAction::Apply {
            commands: vec![ripple(vec![id(3), id(4), id(4)], 1_000_000, 500_000)],
        },
    };
    let receipt = session.execute(request.clone()).unwrap();
    let mut expected = p.clone();
    expected.clips[1].start_us = TimeUs(2_500_000);
    assert_eq!(session.history().project(), &expected);
    assert_eq!(receipt.changed_ids.as_ref().unwrap()["clips"], [id(4)]);
    let mut restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(restored.execute(request.clone()).unwrap(), receipt);
    restored
        .execute(EditRequest {
            operation_id: id(11),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(10),
            },
        })
        .unwrap();
    assert_eq!(restored.history().project(), &p);
    assert_eq!(restored.execute(request).unwrap(), receipt);
}
#[test]
fn boundaries_missing_ids_and_invalid_shifts_reject_whole_batch() {
    let p = fixture();
    for command in [
        ripple(vec![id(3)], 500_000, 1),
        ripple(vec![id(99)], 0, 1),
        ripple(vec![id(3)], 0, -1),
        ripple(vec![id(5)], 0, MAX_TIME_US as i64),
        ripple(vec![id(3)], MAX_TIME_US + 1, 0),
        ripple(vec![id(3)], 0, i64::MIN),
    ] {
        let mut session = EditSession::new(p.clone()).unwrap();
        let before = session.snapshot().unwrap();
        assert!(
            session
                .execute(EditRequest {
                    operation_id: id(10),
                    expected_revision: 0,
                    action: EditAction::Apply {
                        commands: vec![
                            Command::RenameProject {
                                title: "Wrong".into()
                            },
                            command
                        ]
                    }
                })
                .is_err()
        );
        assert_eq!(session.snapshot().unwrap(), before);
    }
    assert_eq!(
        apply_batch(&p, &[ripple(vec![id(3)], 1_000_000, 500_000)]).unwrap(),
        p
    );
}
#[test]
fn scene_resize_is_explicit_and_can_be_in_the_same_batch() {
    let mut p = fixture();
    p.scenes.push(Scene {
        id: id(6),
        name: "Scene".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(5_000_000),
        fade: None,
    });
    for c in &mut p.clips {
        c.scene_id = Some(id(6));
    }
    let command = ripple(vec![id(4), id(5)], 2_000_000, 1_000_000);
    assert!(apply_batch(&p, &[command.clone()]).is_err());
    let result = apply_batch(
        &p,
        &[
            command,
            Command::ResizeScene {
                scene_id: id(6),
                duration_us: TimeUs(6_000_000),
            },
        ],
    )
    .unwrap();
    let mut expected = p;
    expected.clips[1].start_us = TimeUs(3_000_000);
    expected.clips[2].start_us = TimeUs(5_000_000);
    expected.scenes[0].duration_us = TimeUs(6_000_000);
    assert_eq!(result, expected);
}
