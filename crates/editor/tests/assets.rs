use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
fn id(n: u8) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn asset() -> Asset {
    Asset {
        id: id(2),
        name: "透過画像".into(),
        source: Some("手元の素材".into()),
        credit: "作者".into(),
        locator: AssetLocator::Absolute {
            path: std::env::temp_dir()
                .join("missing-image.png")
                .to_str()
                .unwrap()
                .into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Image(ImageProbe {
            format: ImageFormat::Png,
            width: 640,
            height: 480,
            orientation: 1,
            has_alpha: true,
            byte_length: 1234,
            sha256: "a".repeat(64),
        }),
    }
}
fn image_commands() -> Vec<Command> {
    vec![
        Command::RegisterAsset { asset: asset() },
        Command::AddTrack {
            track: Track {
                id: id(3),
                name: "画像".into(),
                kind: TrackKind::Visual,
                visible: true,
                muted: false,
            },
        },
        Command::AddClip {
            clip: Clip {
                id: id(4),
                track_id: id(3),
                scene_id: None,
                start_us: TimeUs(0),
                duration_us: TimeUs(1_000_000),
                transform: Transform {
                    motion: None,
                    x: 0.,
                    y: 0.,
                    width: 640.,
                    height: 480.,
                    opacity: 1.,
                    fade: None,
                    flip_x: None,
                },
                content: ClipContent::Image { asset_id: id(2) },
            },
        },
    ]
}
#[test]
fn registration_roundtrip_and_relink_are_undoable_even_when_source_is_missing() {
    let empty = Project::empty(id(1), "画像編集".into());
    let mut history = History::new(empty.clone()).unwrap();
    history.apply(0, &image_commands()).unwrap();
    let registered = history.project().clone();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&registered).unwrap()).unwrap(),
        registered
    );
    let AssetProbe::Image(mut probe) = asset().probe else {
        panic!()
    };
    probe.sha256 = "b".repeat(64);
    probe.width = 1280;
    history
        .apply(
            1,
            &[Command::RelinkAsset {
                asset_id: id(2),
                locator: AssetLocator::Absolute {
                    path: std::env::temp_dir()
                        .join("new.png")
                        .to_str()
                        .unwrap()
                        .into(),
                },
                probe: AssetProbe::Image(probe),
            }],
        )
        .unwrap();
    assert_eq!(
        history.project().assets[0].content_hash,
        Some("b".repeat(64))
    );
    assert_eq!(history.project().assets[0].credit, "作者");
    history.undo(2).unwrap();
    assert_eq!(history.project(), &registered);
    history.undo(3).unwrap();
    assert_eq!(history.project(), &empty);
    history.redo(4).unwrap();
    assert_eq!(history.project(), &registered);
}
#[test]
fn failed_asset_edits_do_not_leave_dangling_references() {
    let project = apply_batch(&Project::empty(id(1), "画像".into()), &image_commands()).unwrap();
    for commands in [
        vec![Command::RemoveAsset { asset_id: id(2) }],
        vec![Command::RegisterAsset { asset: asset() }],
        vec![Command::SetImageAsset {
            clip_id: id(4),
            asset_id: id(99),
        }],
    ] {
        assert!(apply_batch(&project, &commands).is_err());
    }
    let removed = apply_batch(
        &project,
        &[
            Command::DeleteClip { clip_id: id(4) },
            Command::RemoveAsset { asset_id: id(2) },
        ],
    )
    .unwrap();
    assert!(removed.assets.is_empty() && removed.clips.is_empty());
}
#[test]
fn invalid_metadata_and_unconnected_relative_registration_are_rejected() {
    let p = Project::empty(id(1), "画像".into());
    let mut invalid = asset();
    if let AssetProbe::Image(p) = &mut invalid.probe {
        p.orientation = 9;
    }
    assert!(apply_batch(&p, &[Command::RegisterAsset { asset: invalid }]).is_err());
    let mut invalid = asset();
    invalid.content_hash = Some("b".repeat(64));
    assert!(apply_batch(&p, &[Command::RegisterAsset { asset: invalid }]).is_err());
    let mut invalid = asset();
    invalid.locator = AssetLocator::ProjectRelative {
        path: "a.png".into(),
    };
    assert!(apply_batch(&p, &[Command::RegisterAsset { asset: invalid }]).is_err());
}

#[test]
fn metadata_changes_are_atomic_undoable_and_do_not_change_media() {
    let project = apply_batch(&Project::empty(id(1), "画像".into()), &image_commands()).unwrap();
    let metadata = Command::SetAssetMetadata {
        asset_id: id(2),
        name: "新しい素材名".into(),
        source: None,
        credit: "作者A\n利用条件の記録".into(),
    };
    let mut history = History::new(project.clone()).unwrap();
    history.apply(0, &[metadata.clone()]).unwrap();
    let changed = history.project().clone();
    let mut expected = project.clone();
    expected.assets[0].name = "新しい素材名".into();
    expected.assets[0].source = None;
    expected.assets[0].credit = "作者A\n利用条件の記録".into();
    assert_eq!(changed, expected);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &project);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &changed);
    let invalid = Command::SetAssetMetadata {
        asset_id: id(99),
        name: "missing".into(),
        source: None,
        credit: "".into(),
    };
    assert!(history.apply(3, &[metadata, invalid]).is_err());
    assert_eq!(history.project(), &changed);
    let serialized = serde_json::to_string(&changed).unwrap();
    assert_eq!(
        serde_json::from_str::<Project>(&serialized).unwrap(),
        changed
    );
}

#[test]
fn single_duplicate_shares_asset_keeps_scene_and_z_order_but_not_group() {
    let mut project =
        apply_batch(&Project::empty(id(1), "複製".into()), &image_commands()).unwrap();
    project.scenes.push(Scene {
        fade: None,
        id: id(5),
        name: "導入".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(3_000_000),
    });
    project.clips[0].scene_id = Some(id(5));
    project.groups.push(Group {
        id: id(6),
        name: "まとまり".into(),
        clip_ids: vec![id(4)],
    });
    let mut top = project.clips[0].clone();
    top.id = id(7);
    project.clips.push(top.clone());
    let mut history = History::new(project.clone()).unwrap();
    let command = Command::DuplicateClip {
        clip_id: id(4),
        new_clip_id: id(8),
        start_us: TimeUs(2_000_000),
    };
    history.apply(0, &[command]).unwrap();
    let copied = history.project().clone();
    let mut expected = project.clone();
    let mut copy = project.clips[0].clone();
    copy.id = id(8);
    copy.start_us = TimeUs(2_000_000);
    expected.clips.insert(1, copy);
    assert_eq!(copied, expected);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &project);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &copied);
    for (source, new_id, start) in [(4, 4, 0), (99, 9, 0), (4, 9, 2_000_001), (4, 2, 0)] {
        assert!(
            history
                .apply(
                    3,
                    &[Command::DuplicateClip {
                        clip_id: id(source),
                        new_clip_id: id(new_id),
                        start_us: TimeUs(start)
                    }]
                )
                .is_err()
        );
        assert_eq!(history.project(), &copied);
    }
}
