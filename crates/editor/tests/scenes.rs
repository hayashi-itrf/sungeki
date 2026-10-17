use std::collections::BTreeMap;
use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p = Project::empty(id(1), "Scenes".into());
    for n in 2..4 {
        p.tracks.push(Track {
            id: id(n),
            name: "Track".into(),
            visible: true,
            muted: false,
            kind: TrackKind::Visual,
        });
    }
    p.scenes.push(Scene {
        fade: None,
        id: id(4),
        name: "Intro".into(),
        start_us: TimeUs(100),
        duration_us: TimeUs(100),
    });
    p.scenes.push(Scene {
        fade: None,
        id: id(5),
        name: "End".into(),
        start_us: TimeUs(300),
        duration_us: TimeUs(100),
    });
    for (n, track, scene, start) in [
        (6, 2, Some(4), 100),
        (7, 3, Some(4), 120),
        (8, 2, Some(5), 310),
        (9, 3, None, 0),
    ] {
        p.clips.push(Clip {
            id: id(n),
            track_id: id(track),
            scene_id: scene.map(id),
            start_us: TimeUs(start),
            duration_us: TimeUs(30),
            transform: Transform {
                motion: None,
                x: 10.,
                y: 10.,
                width: 100.,
                height: 100.,
                opacity: 1.,
                fade: None,
                flip_x: None,
            },
            content: ClipContent::Text {
                outline: None,
                layout: None,
                text: format!("Clip {n}"),
                font_size: 20.,
                color: "#000000".into(),
            },
        });
    }
    p.groups.push(Group {
        id: id(10),
        name: "Pair".into(),
        clip_ids: vec![id(6), id(7)],
    });
    p
}
#[test]
fn overlapping_selection_moves_each_clip_once_and_undo_restores() {
    let original = fixture();
    let mut h = History::new(original.clone()).unwrap();
    h.apply(
        0,
        &[Command::MoveSelection {
            scene_ids: vec![id(4), id(4)],
            group_ids: vec![id(10)],
            clip_ids: vec![id(6)],
            delta_us: 50,
        }],
    )
    .unwrap();
    assert_eq!(h.project().scenes[0].start_us, TimeUs(150));
    assert_eq!(
        h.project()
            .clips
            .iter()
            .map(|c| c.start_us.0)
            .collect::<Vec<_>>(),
        vec![150, 170, 310, 0]
    );
    assert_eq!(h.project().groups, original.groups);
    assert_eq!(h.project().tracks, original.tracks);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &original);
}
#[test]
fn layout_reorders_scenes_without_moving_independent_clips() {
    let original = fixture();
    let result = apply_batch(
        &original,
        &[Command::LayoutScenes {
            scene_ids: vec![id(5), id(4)],
            start_us: TimeUs(0),
            gap_us: TimeUs(20),
        }],
    )
    .unwrap();
    assert_eq!(result.scenes[0].start_us, TimeUs(120));
    assert_eq!(result.scenes[1].start_us, TimeUs(0));
    assert_eq!(
        result
            .clips
            .iter()
            .map(|c| c.start_us.0)
            .collect::<Vec<_>>(),
        vec![120, 140, 10, 0]
    );
    assert_eq!(result.clips[3], original.clips[3]);
}
#[test]
fn invalid_move_resize_layout_and_extreme_times_are_atomic() {
    let original = fixture();
    let mut h = History::new(original.clone()).unwrap();
    let cases = vec![
        Command::MoveScene {
            scene_id: id(4),
            start_us: TimeUs(250),
        },
        Command::ResizeScene {
            scene_id: id(4),
            duration_us: TimeUs(10),
        },
        Command::MoveSelection {
            scene_ids: vec![id(4)],
            group_ids: vec![],
            clip_ids: vec![],
            delta_us: -101,
        },
        Command::MoveGroup {
            group_id: id(10),
            start_us: TimeUs(u64::MAX),
        },
        Command::MoveGroup {
            group_id: id(10),
            start_us: TimeUs(190),
        },
        Command::LayoutScenes {
            scene_ids: vec![id(4), id(4)],
            start_us: TimeUs(0),
            gap_us: TimeUs(0),
        },
    ];
    for command in cases {
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "Not committed".into()
                    },
                    command
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &original);
        assert_eq!(h.revision(), 0);
        assert_eq!(h.undo_len(), 0);
    }
}
#[test]
fn deleting_scene_removes_members_but_ungrouping_keeps_times_and_groups() {
    let original = fixture();
    let removed = apply_batch(&original, &[Command::DeleteScene { scene_id: id(4) }]).unwrap();
    assert_eq!(
        removed
            .clips
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>(),
        vec![id(8), id(9)]
    );
    assert!(removed.groups.is_empty());
    let detached = apply_batch(&original, &[Command::UngroupScene { scene_id: id(4) }]).unwrap();
    assert_eq!(detached.groups, original.groups);
    assert_eq!(detached.clips[0].start_us, original.clips[0].start_us);
    assert_eq!(detached.clips[1].scene_id, None);
}
#[test]
fn duplication_maps_all_ids_and_preserves_order_membership_and_source() {
    let original = fixture();
    let command = Command::DuplicateScene {
        scene_id: id(4),
        new_scene_id: id(20),
        start_us: TimeUs(500),
        clip_ids: BTreeMap::from([(id(6), id(22)), (id(7), id(21))]),
        group_ids: BTreeMap::from([(id(10), id(23))]),
    };
    let result = apply_batch(&original, &[command]).unwrap();
    assert_eq!(&result.clips[..4], &original.clips);
    assert_eq!(result.clips[4].id, id(22));
    assert_eq!(result.clips[5].id, id(21));
    assert_eq!(result.clips[4].start_us, TimeUs(500));
    assert_eq!(result.clips[5].start_us, TimeUs(520));
    assert_eq!(result.clips[4].scene_id, Some(id(20)));
    assert_eq!(result.groups[1].clip_ids, vec![id(22), id(21)]);
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&result).unwrap()).unwrap();
    assert_eq!(saved, result);
    for ids in [
        BTreeMap::from([(id(6), id(30))]),
        BTreeMap::from([(id(6), id(30)), (id(7), id(30))]),
    ] {
        assert!(
            apply_batch(
                &original,
                &[Command::DuplicateGroup {
                    group_id: id(10),
                    new_group_id: id(31),
                    start_us: TimeUs(140),
                    clip_ids: ids
                }]
            )
            .is_err()
        );
    }
    let copied = apply_batch(
        &original,
        &[Command::DuplicateGroup {
            group_id: id(10),
            new_group_id: id(31),
            start_us: TimeUs(140),
            clip_ids: BTreeMap::from([(id(6), id(32)), (id(7), id(33))]),
        }],
    )
    .unwrap();
    assert_eq!(copied.clips[5].start_us, TimeUs(160));
    assert_eq!(copied.clips[5].scene_id, Some(id(4)));
}

#[test]
fn rename_resize_and_group_move_are_one_undoable_edit() {
    let original = fixture();
    let mut history = History::new(original.clone()).unwrap();
    history
        .apply(
            0,
            &[
                Command::RenameScene {
                    scene_id: id(4),
                    name: "Opening".into(),
                },
                Command::ResizeScene {
                    scene_id: id(4),
                    duration_us: TimeUs(150),
                },
                Command::RenameGroup {
                    group_id: id(10),
                    name: "Dialogue".into(),
                },
                Command::MoveGroup {
                    group_id: id(10),
                    start_us: TimeUs(150),
                },
            ],
        )
        .unwrap();
    assert_eq!(history.project().scenes[0].name, "Opening");
    assert_eq!(history.project().groups[0].name, "Dialogue");
    assert_eq!(history.project().clips[0].start_us, TimeUs(150));
    assert_eq!(history.project().clips[1].start_us, TimeUs(170));
    assert_eq!(history.project().scenes[0].start_us, TimeUs(100));
    history.undo(1).unwrap();
    assert_eq!(history.project(), &original);
}
