use sungeki_editor::{Command, History};
use sungeki_project::*;
fn asset() -> Asset {
    Asset {
        id: "00000000-0000-4000-8000-000000000002".into(),
        name: "MP3 BGM".into(),
        source: None,
        credit: "test".into(),
        content_hash: Some("a".repeat(64)),
        locator: AssetLocator::Absolute {
            path: "/missing/music.mp3".into(),
        },
        probe: AssetProbe::Audio(AudioProbe {
            encoding: Some(CompressedAudioEncoding::Mp3 {
                decode_version: 1,
                decoded_sha256: "b".repeat(64),
            }),
            sha256: "a".repeat(64),
            byte_length: 16000,
            sample_rate: 48000,
            channels: 2,
            frames: 480000,
            duration_us: TimeUs(10_000_000),
        }),
    }
}
#[test]
fn compressed_identity_survives_save_undo_redo() {
    let empty = Project::empty("00000000-0000-4000-8000-000000000001".into(), "MP3".into());
    let mut history = History::new(empty.clone()).unwrap();
    history
        .apply(0, &[Command::RegisterAsset { asset: asset() }])
        .unwrap();
    let registered = history.project().clone();
    let loaded: Project =
        serde_json::from_slice(&serde_json::to_vec(&registered).unwrap()).unwrap();
    assert_eq!(loaded, registered);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &empty);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &registered);
}
#[test]
fn invalid_decoder_clock_hash_and_unmarked_compression_are_atomic_errors() {
    for case in 0..6 {
        let mut asset = asset();
        let AssetProbe::Audio(p) = &mut asset.probe else {
            unreachable!()
        };
        match case {
            0 => p.encoding = None,
            1 => p.sample_rate = 44100,
            2 => {
                p.encoding = Some(CompressedAudioEncoding::Mp3 {
                    decode_version: 2,
                    decoded_sha256: "b".repeat(64),
                })
            }
            3 => {
                p.encoding = Some(CompressedAudioEncoding::Mp3 {
                    decode_version: 1,
                    decoded_sha256: "x".repeat(64),
                })
            }
            4 => {
                p.frames = 48000 * 7200 + 1;
                p.duration_us = TimeUs((p.frames * 1_000_000).div_ceil(48000));
            }
            _ => p.duration_us = TimeUs(9_999_999),
        }
        let empty = Project::empty("00000000-0000-4000-8000-000000000001".into(), "MP3".into());
        let mut h = History::new(empty.clone()).unwrap();
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "must not apply".into()
                    },
                    Command::RegisterAsset { asset }
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &empty);
    }
}

#[test]
fn long_compressed_sources_roundtrip_through_editor() {
    for seconds in [180, 7200] {
        let mut a = asset();
        let AssetProbe::Audio(p) = &mut a.probe else {
            unreachable!()
        };
        p.frames = 48000 * seconds;
        p.duration_us = TimeUs(seconds * 1_000_000);
        let empty = Project::empty(
            "00000000-0000-4000-8000-000000000001".into(),
            "long MP3".into(),
        );
        let mut h = History::new(empty.clone()).unwrap();
        h.apply(0, &[Command::RegisterAsset { asset: a }]).unwrap();
        let loaded: Project =
            serde_json::from_slice(&serde_json::to_vec(h.project()).unwrap()).unwrap();
        assert_eq!(&loaded, h.project());
        h.undo(1).unwrap();
        assert_eq!(h.project(), &empty);
        h.redo(2).unwrap();
        assert_eq!(h.project(), &loaded);
    }
}
