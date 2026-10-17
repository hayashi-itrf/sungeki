use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
fn id(n: u8) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn audio() -> Asset {
    Asset {
        id: id(2),
        name: "生成音声".into(),
        source: None,
        credit: "話者".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/original.wav".into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(10_000),
            sample_rate: 24000,
            channels: 1,
            frames: 240,
            byte_length: 524,
        }),
    }
}
#[test]
fn audio_registration_persists_through_history_and_serialization() {
    let empty = Project::empty(id(1), "音声原本".into());
    let mut history = History::new(empty.clone()).unwrap();
    history
        .apply(0, &[Command::RegisterAsset { asset: audio() }])
        .unwrap();
    let registered = history.project().clone();
    let json = serde_json::to_vec(&registered).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&json).unwrap(),
        registered
    );
    history.undo(1).unwrap();
    assert_eq!(history.project(), &empty);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &registered);
}
#[test]
fn malformed_measurements_and_hashes_reject_the_whole_batch() {
    let empty = Project::empty(id(1), "原本".into());
    for case in 0..8 {
        let mut asset = audio();
        let AssetProbe::Audio(probe) = &mut asset.probe else {
            unreachable!()
        };
        match case {
            0 => probe.sample_rate = 0,
            1 => probe.frames = 0,
            2 => probe.frames = u64::MAX,
            3 => probe.duration_us = TimeUs(9_999),
            4 => probe.channels = 3,
            5 => probe.byte_length = 64 * 1024 * 1024 + 1,
            6 => probe.sha256 = "b".repeat(64),
            _ => probe.byte_length = 44,
        }
        assert!(
            apply_batch(
                &empty,
                &[
                    Command::RenameProject {
                        title: "変更しない".into()
                    },
                    Command::RegisterAsset { asset }
                ]
            )
            .is_err()
        );
    }
    assert_eq!(empty.title, "原本");
}
#[test]
fn image_clip_cannot_bind_an_audio_asset() {
    let p = Project::empty(id(1), "素材の種類".into());
    let commands = [
        Command::RegisterAsset { asset: audio() },
        Command::AddTrack {
            track: Track {
                id: id(3),
                name: "映像".into(),
                visible: true,
                muted: false,
                kind: TrackKind::Visual,
            },
        },
        Command::AddClip {
            clip: Clip {
                id: id(4),
                track_id: id(3),
                scene_id: None,
                start_us: TimeUs(0),
                duration_us: TimeUs(10_000),
                transform: Transform {
                    motion: None,
                    x: 0.,
                    y: 0.,
                    width: 10.,
                    height: 10.,
                    opacity: 1.,
                    fade: None,
                    flip_x: None,
                },
                content: ClipContent::Image { asset_id: id(2) },
            },
        },
    ];
    assert!(apply_batch(&p, &commands).is_err());
}
