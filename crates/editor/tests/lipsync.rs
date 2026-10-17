use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;
fn timing() -> LipSyncAnalysis {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/lipsync-analysis.json"
    ))
    .unwrap()
}
fn reference() -> SpeechAudio {
    SpeechAudio {
        irodori_source: None,
        qwen_source: None,
        sbv2_source: None,
        asset_id: id(6),
        input_fingerprint: "b".repeat(64),
        engine_version: "test".into(),
        lip_sync: Some(timing()),
    }
}
fn fixture() -> Project {
    let mut p = apply_batch(&Project::empty(id(1), "口パク保存".into()), &commands()).unwrap();
    p.assets.push(Asset {
        id: id(6),
        name: "原本".into(),
        source: None,
        credit: "話者".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/original.wav".into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(1_000_000),
            sample_rate: 24000,
            channels: 1,
            frames: 24000,
            byte_length: 48044,
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
#[test]
fn shared_edit_roundtrip_undo_and_relevant_input_invalidation() {
    let project = fixture();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&project).unwrap()).unwrap(),
        project
    );
    let mut history = History::new(project.clone()).unwrap();
    let mut audio = reference();
    audio.lip_sync = None;
    history
        .apply(
            0,
            &[Command::SetSpeechAudio {
                clip_id: id(4),
                audio: Some(audio),
            }],
        )
        .unwrap();
    assert_ne!(history.project(), &project);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &project);
    history
        .apply(
            2,
            &[Command::SetSpeech {
                clip_id: id(4),
                character_id: id(2),
                subtitle_text: "新しい発話".into(),
                spoken_text: None,
            }],
        )
        .unwrap();
    assert!(matches!(
        history.project().clips[0].content,
        ClipContent::Speech { audio: None, .. }
    ));
    history.undo(3).unwrap();
    assert_eq!(history.project(), &project);
}
#[test]
fn stale_original_and_invalid_intervals_reject_entire_edit() {
    let project = fixture();
    for mutation in 0..10 {
        let mut audio = reference();
        let t = audio.lip_sync.as_mut().unwrap();
        match mutation {
            0 => t.source_sha256 = "c".repeat(64),
            1 => t.sample_rate = 48000,
            2 => t.source_frames += 1,
            3 => t.version = 2,
            4 => t.open.push(t.open[0]),
            5 => t.open[0].end_frame = 24001,
            6 => t.open[0].start_frame = t.open[0].end_frame,
            7 => t.threshold_rms = f64::NAN,
            8 => t.peak_rms = f64::MAX,
            _ => t.settings.window_us = 0,
        }
        let mut history = History::new(project.clone()).unwrap();
        assert!(
            history
                .apply(
                    0,
                    &[
                        Command::RenameProject {
                            title: "反映しない".into()
                        },
                        Command::SetSpeechAudio {
                            clip_id: id(4),
                            audio: Some(audio)
                        }
                    ]
                )
                .is_err(),
            "{mutation}"
        );
        assert_eq!(history.project(), &project);
    }
    let mut changed = project.clone();
    let AssetProbe::Audio(probe) = &mut changed.assets[0].probe else {
        panic!()
    };
    probe.sha256 = "c".repeat(64);
    changed.assets[0].content_hash = Some("c".repeat(64));
    assert!(validate(&changed).is_err());
}
#[test]
fn untrusted_timing_rejects_unknown_fields_and_inconsistent_measurements() {
    let mut json = serde_json::to_value(timing()).unwrap();
    json["unrecognized"] = true.into();
    assert!(serde_json::from_value::<LipSyncAnalysis>(json).is_err());
    let project = fixture();
    let AssetProbe::Audio(probe) = &project.assets[0].probe else {
        panic!()
    };
    for mutation in 0..4 {
        let mut t = timing();
        match mutation {
            0 => t.threshold_rms = 0.2,
            1 => {
                t.peak_rms = 0.;
                t.threshold_rms = 0.01;
            }
            2 => t.open[0].end_frame = t.open[0].start_frame + 1,
            _ => t.peak_rms = f64::INFINITY,
        }
        assert!(t.validate(probe).is_err());
    }
}
