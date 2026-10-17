use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p = Project::empty(id(1), "編集前".into());
    p.tracks.push(Track {
        id: id(2),
        name: "字幕".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Visual,
    });
    p.clips.push(Clip {
        id: id(3),
        track_id: id(2),
        scene_id: None,
        start_us: TimeUs(0),
        duration_us: TimeUs(100),
        transform: Transform {
            motion: None,
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.,
            opacity: 1.,
            fade: None,
            flip_x: None,
        },
        content: ClipContent::Text {
            outline: None,
            layout: None,
            text: "前".into(),
            font_size: 20.,
            color: "#000000".into(),
        },
    });
    p
}
#[test]
fn failed_batch_never_changes_original_or_history() {
    let p = fixture();
    let mut history = History::new(p.clone()).unwrap();
    assert!(
        history
            .apply(
                0,
                &[
                    Command::RenameProject {
                        title: "途中".into()
                    },
                    Command::DeleteClip { clip_id: id(99) }
                ]
            )
            .is_err()
    );
    assert_eq!(history.project(), &p);
    assert_eq!(history.revision(), 0);
    assert_eq!(history.undo_len(), 0);
}
#[test]
fn batch_is_one_undo_and_redo_then_new_edit_clears_redo() {
    let p = fixture();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[
            Command::RenameProject {
                title: "後".into()
            },
            Command::SetText {
                clip_id: id(3),
                text: "変更後".into(),
            },
        ],
    )
    .unwrap();
    let after = h.project().clone();
    assert_eq!(h.undo_len(), 1);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    assert_eq!(h.revision(), 2);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &after);
    assert_eq!(h.revision(), 3);
    h.undo(3).unwrap();
    h.apply(
        4,
        &[Command::RenameProject {
            title: "分岐".into(),
        }],
    )
    .unwrap();
    assert_eq!(h.redo_len(), 0);
}
#[test]
fn stale_revision_is_rejected_and_noop_preserves_redo() {
    let mut h = History::new(fixture()).unwrap();
    h.apply(
        0,
        &[Command::RenameProject {
            title: "後".into()
        }],
    )
    .unwrap();
    assert!(matches!(h.apply(0, &[]), Err(HistoryError::Conflict)));
    h.undo(1).unwrap();
    assert!(!h.apply(2, &[]).unwrap());
    assert_eq!(h.revision(), 2);
    assert_eq!(h.redo_len(), 1);
    assert!(matches!(h.undo(1), Err(HistoryError::Conflict)));
}
#[test]
fn scene_move_preserves_relative_times_and_collision_rolls_back() {
    let mut p = fixture();
    p.scenes.push(Scene {
        fade: None,
        id: id(4),
        name: "導入".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(100),
    });
    p.clips[0].scene_id = Some(id(4));
    p.scenes.push(Scene {
        fade: None,
        id: id(5),
        name: "次".into(),
        start_us: TimeUs(1000),
        duration_us: TimeUs(100),
    });
    let moved = apply_batch(
        &p,
        &[Command::MoveScene {
            scene_id: id(4),
            start_us: TimeUs(200),
        }],
    )
    .unwrap();
    assert_eq!(moved.clips[0].start_us, TimeUs(200));
    assert!(
        apply_batch(
            &p,
            &[Command::MoveScene {
                scene_id: id(4),
                start_us: TimeUs(950)
            }]
        )
        .is_err()
    );
    assert_eq!(p.clips[0].start_us, TimeUs(0));
}
#[test]
fn deleting_last_grouped_clip_removes_group_and_undo_restores_it() {
    let mut p = fixture();
    p.groups.push(Group {
        id: id(4),
        name: "まとまり".into(),
        clip_ids: vec![id(3)],
    });
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[Command::DeleteClip { clip_id: id(3) }])
        .unwrap();
    assert!(h.project().groups.is_empty());
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
}
#[test]
fn history_retains_only_last_hundred_operations() {
    let mut h = History::new(fixture()).unwrap();
    for revision in 0..105 {
        h.apply(
            revision,
            &[Command::RenameProject {
                title: revision.to_string(),
            }],
        )
        .unwrap();
    }
    assert_eq!(h.undo_len(), 100);
    for _ in 0..100 {
        h.undo(h.revision()).unwrap();
    }
    assert_eq!(h.project().title, "4");
    assert!(matches!(h.undo(h.revision()), Err(HistoryError::Empty)));
}
#[test]
fn final_validation_allows_explicit_detach_in_same_batch() {
    let mut p = fixture();
    p.scenes.push(Scene {
        fade: None,
        id: id(4),
        name: "導入".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(100),
    });
    p.clips[0].scene_id = Some(id(4));
    let commands = [
        Command::MoveClip {
            clip_id: id(3),
            start_us: TimeUs(1000),
        },
        Command::SetClipScene {
            clip_id: id(3),
            scene_id: None,
        },
    ];
    assert!(apply_batch(&p, &commands[..1]).is_err());
    assert!(apply_batch(&p, &commands).is_ok());
}
#[test]
fn shape_rejects_text_patch_and_unknown_commands() {
    let mut p = fixture();
    p.clips[0].content = ClipContent::Shape {
        shape: ShapeKind::Ellipse,
        fill: "#abcdef".into(),
    };
    assert!(
        apply_batch(
            &p,
            &[Command::SetText {
                clip_id: id(3),
                text: "不正".into()
            }]
        )
        .is_err()
    );
    assert!(serde_json::from_str::<Command>(r#"{"type":"unknown"}"#).is_err());
    assert!(
        serde_json::from_str::<Command>(r#"{"type":"rename_project","title":"x","extra":true}"#)
            .is_err()
    );
}

#[test]
fn history_byte_limit_prunes_old_snapshots() {
    let mut h = History::new(fixture()).unwrap();
    let large = "a".repeat(33 * 1024 * 1024);
    h.apply(0, &[Command::RenameProject { title: large }])
        .unwrap();
    h.apply(
        1,
        &[Command::SetText {
            clip_id: id(3),
            text: "2".into(),
        }],
    )
    .unwrap();
    h.apply(
        2,
        &[Command::SetText {
            clip_id: id(3),
            text: "3".into(),
        }],
    )
    .unwrap();
    assert_eq!(h.undo_len(), 1); // Two 33MiB snapshots cannot fit the 64MiB budget.
    h.undo(3).unwrap();
    assert_eq!(h.undo_len(), 0);
    h.redo(4).unwrap();
    match &h.project().clips[0].content {
        ClipContent::Text { text, .. } => assert_eq!(text, "3"),
        _ => panic!(),
    }
}

#[test]
fn add_text_and_shape_in_one_batch_preserves_drawing_order() {
    let mut p = fixture();
    let mut text = p.clips.remove(0);
    let mut shape = text.clone();
    shape.id = id(6);
    let mut track = p.tracks[0].clone();
    track.id = id(7);
    p.tracks.push(track);
    text.track_id = id(7);
    shape.content = ClipContent::Shape {
        shape: ShapeKind::Rectangle,
        fill: "#aabbcc".into(),
    };
    let candidate = apply_batch(
        &p,
        &[
            Command::AddClip { clip: shape },
            Command::AddClip { clip: text },
        ],
    )
    .unwrap();
    assert_eq!(
        candidate
            .clips
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>(),
        vec![id(6), id(3)]
    );
}
