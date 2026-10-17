use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
fn fixture() -> Project {
    let mut p = apply_batch(&Project::empty(id(1), "生成音声参照".into()), &commands()).unwrap();
    p.assets.push(Asset {
        id: id(6),
        name: "生成音声".into(),
        source: None,
        credit: "話者".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/generated.wav".into(),
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
    });
    apply_batch(
        &p,
        &[Command::SetSpeechAudio {
            clip_id: id(4),
            audio: Some(reference()),
        }],
    )
    .unwrap()
}
fn reference() -> SpeechAudio {
    SpeechAudio {
        irodori_source: None,
        qwen_source: None,
        sbv2_source: None,
        asset_id: id(6),
        input_fingerprint: "b".repeat(64),
        engine_version: "1.0".into(),
        lip_sync: None,
    }
}

fn layout(ids: Vec<String>, start: u64, gap: u64) -> Command {
    Command::LayoutSpeech {
        clip_ids: ids,
        start_us: TimeUs(start),
        gap_us: TimeUs(gap),
    }
}
#[test]
fn layout_uses_original_duration_and_explicit_order_and_undo() {
    let p = apply_batch(
        &fixture(),
        &[Command::DuplicateClip {
            clip_id: id(4),
            new_clip_id: id(7),
            start_us: TimeUs(3_000_000),
        }],
    )
    .unwrap();
    let mut h = History::new(p.clone()).unwrap();
    h.apply(0, &[layout(vec![id(7), id(4)], 100_000, 350_000)])
        .unwrap();
    assert_eq!(h.project().clips[1].start_us, TimeUs(100_000));
    assert_eq!(h.project().clips[0].start_us, TimeUs(460_000));
    for (before, after) in p.clips.iter().zip(&h.project().clips) {
        assert_eq!(after.duration_us, TimeUs(10_000));
        assert_eq!(after.content, before.content);
        assert_eq!(after.scene_id, before.scene_id);
    }
    assert_eq!(h.project().assets, p.assets);
    assert_eq!(h.project().scenes, p.scenes);
    let next = h.project().clone();
    h.undo(1).unwrap();
    assert_eq!(h.project(), &p);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &next);
}
#[test]
fn invalid_layout_is_atomic() {
    for command in [
        layout(vec![], 0, 0),
        layout(vec![id(4), id(4)], 0, 0),
        layout(vec![id(99)], 0, 0),
        layout(vec![id(4)], MAX_TIME_US, 0),
        layout(vec![id(4)], 5_000_000, 0),
        layout(vec![id(4)], 0, MAX_TIME_US + 1),
    ] {
        let p = fixture();
        let mut h = History::new(p.clone()).unwrap();
        assert!(
            h.apply(
                0,
                &[
                    Command::RenameProject {
                        title: "rollback".into()
                    },
                    command
                ]
            )
            .is_err()
        );
        assert_eq!(h.project(), &p);
    }
    let p = apply_batch(
        &fixture(),
        &[Command::SetSpeechAudio {
            clip_id: id(4),
            audio: None,
        }],
    )
    .unwrap();
    assert!(apply_batch(&p, &[layout(vec![id(4)], 0, 0)]).is_err());
}
#[test]
fn unselected_speech_overlap_is_rejected_but_adjacent_is_allowed() {
    let p = apply_batch(
        &fixture(),
        &[Command::DuplicateClip {
            clip_id: id(4),
            new_clip_id: id(7),
            start_us: TimeUs(3_000_000),
        }],
    )
    .unwrap();
    assert!(apply_batch(&p, &[layout(vec![id(4)], 3_000_000, 0)]).is_err());
    let next = apply_batch(&p, &[layout(vec![id(4)], 2_990_000, 0)]).unwrap();
    assert_eq!(next.clips[1], p.clips[1]);
}
