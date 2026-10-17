use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;

fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    let mut p = Project::empty(id(1), "重なりの拒否".into());
    for n in [2, 3] {
        p.tracks.push(Track {
            id: id(n),
            name: format!("図形{n}"),
            kind: TrackKind::Visual,
            visible: true,
            muted: false,
        });
    }
    for (n, start) in [(4, 0), (5, 2_000_000)] {
        p.clips.push(
            serde_json::from_value(serde_json::json!({
                "id": id(n), "track_id": id(2), "scene_id": null,
                "start_us": start, "duration_us": 1_000_000,
                "transform": {"x":0,"y":0,"width":100,"height":100,"opacity":1},
                "content":{"type":"shape","shape":"rectangle","fill":"#00ff00"}
            }))
            .unwrap(),
        );
    }
    p
}

#[test]
fn move_resize_and_add_reject_overlap_atomically() {
    let p = fixture();
    let mut added = p.clips[0].clone();
    added.id = id(6);
    for command in [
        Command::MoveClip {
            clip_id: id(4),
            start_us: TimeUs(1_500_000),
        },
        Command::ResizeClip {
            clip_id: id(4),
            duration_us: TimeUs(2_000_001),
        },
        Command::AddClip { clip: added },
    ] {
        let mut h = History::new(p.clone()).unwrap();
        let result = h.apply(
            0,
            &[
                Command::RenameProject {
                    title: "失敗時に残らない".into(),
                },
                command,
            ],
        );
        assert!(result.is_err());
        assert_eq!(h.project(), &p);
        assert_eq!(h.revision(), 0);
        assert_eq!(h.undo_len(), 0);
    }
}

#[test]
fn boundary_touch_and_final_batch_swap_are_allowed() {
    let p = fixture();
    let touching = apply_batch(
        &p,
        &[Command::ResizeClip {
            clip_id: id(4),
            duration_us: TimeUs(2_000_000),
        }],
    )
    .unwrap();
    assert_eq!(touching.clips[0].duration_us, TimeUs(2_000_000));
    let mut h = History::new(p.clone()).unwrap();
    h.apply(
        0,
        &[
            Command::MoveClip {
                clip_id: id(4),
                start_us: TimeUs(2_000_000),
            },
            Command::MoveClip {
                clip_id: id(5),
                start_us: TimeUs(0),
            },
        ],
    )
    .unwrap();
    let swapped = h.project().clone();
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &swapped);
}

#[test]
fn simultaneous_clips_are_allowed_on_separate_tracks_but_transfer_back_is_rejected() {
    let p = fixture();
    let separated = apply_batch(
        &p,
        &[
            Command::MoveClip {
                clip_id: id(5),
                start_us: TimeUs(0),
            },
            Command::SetClipTrack {
                clip_id: id(5),
                track_id: id(3),
            },
        ],
    )
    .unwrap();
    let error = apply_batch(
        &separated,
        &[Command::SetClipTrack {
            clip_id: id(5),
            track_id: id(2),
        }],
    )
    .unwrap_err();
    assert!(error.0.contains("図形2"));
    assert!(error.0.contains("別のトラック"));
}

#[test]
fn same_time_duplicate_uses_a_separate_layer_and_undo_restores_tracks() {
    let p = fixture();
    let mut h = History::new(p.clone()).unwrap();
    let command = Command::DuplicateClip {
        clip_id: id(4),
        new_clip_id: id(6),
        start_us: TimeUs(0),
    };
    let preview = apply_batch(&p, &[command.clone()]).unwrap();
    h.apply(0, &[command]).unwrap();
    assert_eq!(h.project(), &preview);
    assert_eq!(preview.tracks.len(), 3);
    assert_eq!(preview.clips[1].start_us, p.clips[0].start_us);
    assert_ne!(preview.clips[1].track_id, p.clips[0].track_id);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
}

#[test]
fn untouched_existing_overlap_does_not_block_text_but_enclosing_overlap_is_checked() {
    let mut p = fixture();
    p.clips[0].duration_us = TimeUs(10_000_000);
    let mut added = p.clips[1].clone();
    added.id = id(6);
    added.start_us = TimeUs(5_000_000);
    assert!(
        apply_batch(
            &p,
            &[Command::RenameProject {
                title: "改名".into()
            }]
        )
        .is_ok()
    );
    assert!(apply_batch(&p, &[Command::AddClip { clip: added }]).is_err());
}

#[test]
fn group_scene_selection_and_ripple_cannot_bypass_collision_check() {
    let mut p = fixture();
    p.scenes.push(Scene {
        id: id(10),
        name: "場面".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        fade: None,
    });
    p.clips[0].scene_id = Some(id(10));
    p.groups.push(Group {
        id: id(11),
        name: "グループ".into(),
        clip_ids: vec![id(5)],
    });
    for command in [
        Command::MoveScene {
            scene_id: id(10),
            start_us: TimeUs(2_000_000),
        },
        Command::MoveGroup {
            group_id: id(11),
            start_us: TimeUs(0),
        },
        Command::MoveSelection {
            scene_ids: vec![],
            group_ids: vec![],
            clip_ids: vec![id(5)],
            delta_us: -2_000_000,
        },
        Command::Ripple {
            clip_ids: vec![id(5)],
            boundary_us: TimeUs(2_000_000),
            delta_us: -2_000_000,
        },
    ] {
        let mut h = History::new(p.clone()).unwrap();
        assert!(h.apply(0, &[command]).is_err());
        assert_eq!(h.project(), &p);
        assert_eq!(h.revision(), 0);
    }
}
