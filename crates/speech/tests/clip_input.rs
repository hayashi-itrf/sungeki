#[allow(dead_code)]
mod http_support;
use sungeki_project::*;
use sungeki_speech::{PreparedSpeechClip, VoicevoxIdentity};
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn identity() -> VoicevoxIdentity {
    VoicevoxIdentity {
        engine_id: "test".into(),
        engine_version: "1".into(),
        core_version: "0.16.0".into(),
    }
}
fn project() -> Project {
    let mut project = Project::empty(id(1), "入力固定".into());
    project.characters.push(Character {
        id: id(2),
        name: "話者".into(),
        credit: "検証".into(),
        subtitle_style: SubtitleStyle {
            layout: None,
            outline: None,
            font_size: 64.0,
            color: "#000000".into(),
        },
        voice: Voice::Voicevox {
            engine_id: "test".into(),
            core_version: "0.16.0".into(),
            settings: http_support::settings(),
        },
    });
    project.tracks.push(Track {
        id: id(3),
        name: "発話".into(),
        visible: true,
        muted: false,
        kind: TrackKind::Audio,
    });
    project.clips.push(Clip {
        id: id(4),
        track_id: id(3),
        scene_id: None,
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: Transform {
            motion: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            opacity: 1.0,
            fade: None,
            flip_x: None,
        },
        content: ClipContent::Speech {
            gain_db: None,
            audio: None,
            character_id: id(2),
            subtitle_text: "字幕".into(),
            spoken_text: Some("よみ".into()),
        },
    });
    project
}
#[test]
fn fixed_target_detects_edits_deletion_and_other_project_but_allows_background_changes() {
    let original = project();
    let fixed = PreparedSpeechClip::new(&original, &id(4), &identity()).unwrap();
    assert_eq!(fixed.audio().resolved().text, "よみ");
    assert!(fixed.matches(&original, &identity()));
    let mut unrelated = original.clone();
    unrelated.canvas.background = "#112233".into();
    assert!(fixed.matches(&unrelated, &identity()));
    for mutation in 0..7 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.clips.clear(),
            1 => changed.clips[0].start_us = TimeUs(1),
            2 => changed.clips[0].duration_us = TimeUs(2_000_000),
            3 => changed.clips[0].transform.x = 50.0,
            4 => changed.characters[0].subtitle_style.font_size = 72.0,
            5 => changed.id = id(99),
            _ => changed.pronunciation_dictionary.push(PronunciationEntry {
                written: "字幕".into(),
                spoken: "じまく".into(),
            }),
        }
        assert!(!fixed.matches(&changed, &identity()), "mutation {mutation}");
    }
    let mut changed_engine = identity();
    changed_engine.engine_version = "2".into();
    assert!(!fixed.matches(&original, &changed_engine));
    let mut wrong_engine = identity();
    wrong_engine.engine_id = "other".into();
    assert!(PreparedSpeechClip::new(&original, &id(4), &wrong_engine).is_err());
}

#[test]
fn durable_job_roundtrip_revalidates_input_without_mutating_project() {
    use sungeki_speech::SpeechJobInput;
    let mut project = project();
    let input =
        SpeechJobInput::new(7, &project, &id(4), "http://127.0.0.1:55021", &identity()).unwrap();
    let bytes = serde_json::to_vec(&input).unwrap();
    let restored: SpeechJobInput = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.source_revision(), 7);
    assert_eq!(restored.endpoint(), "http://127.0.0.1:55021/");
    assert_eq!(restored.input_hash(), input.input_hash());
    assert_eq!(restored.prepared().unwrap().audio().resolved().text, "よみ");
    project.characters.clear();
    assert_eq!(restored.project().characters.len(), 1);
    for pointer in [
        "/format_version",
        "/lip_sync_version",
        "/lip_sync_settings/window_us",
        "/source_revision",
        "/endpoint",
        "/clip_id",
        "/identity/engine_version",
        "/input_hash",
        "/project/title",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let field = value.pointer_mut(pointer).unwrap();
        *field = if field.is_number() {
            serde_json::json!(999)
        } else {
            serde_json::json!("changed")
        };
        assert!(
            serde_json::from_value::<SpeechJobInput>(value).is_err(),
            "{pointer}"
        );
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<SpeechJobInput>(value).is_err());
    assert!(
        SpeechJobInput::new(
            0,
            restored.project(),
            &id(4),
            "http://example.com/",
            &identity()
        )
        .is_err()
    );
}

#[test]
fn persisted_job_preserves_precise_floating_point_clip_positions() {
    use sungeki_speech::SpeechJobInput;
    let mut project = project();
    for n in 1..256 {
        project.clips[0].transform.x = 1.0 + f64::from(n) / 997.0;
        let input = SpeechJobInput::new(1, &project, &id(4), "http://127.0.0.1:55021", &identity())
            .unwrap();
        let serialized = serde_json::to_vec(&input).unwrap();
        let restored: SpeechJobInput = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(
            restored.project().clips[0].transform.x.to_bits(),
            project.clips[0].transform.x.to_bits()
        );
    }
}

#[test]
fn saved_audio_fingerprint_detects_stale_inputs_but_not_placement_or_style() {
    use sungeki_speech::audio_reference_matches;
    let mut p = project();
    let fixed = PreparedSpeechClip::new(&p, &id(4), &identity()).unwrap();
    p.assets.push(Asset {
        id: id(5),
        name: "音声".into(),
        source: None,
        credit: "".into(),
        locator: AssetLocator::Absolute {
            path: "/missing.wav".into(),
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
    let ClipContent::Speech { audio, .. } = &mut p.clips[0].content else {
        panic!()
    };
    *audio = Some(SpeechAudio {
        irodori_source: None,
        qwen_source: None,
        sbv2_source: None,
        asset_id: id(5),
        input_fingerprint: fixed.audio().fingerprint().into(),
        engine_version: identity().engine_version,
        lip_sync: None,
    });
    assert!(audio_reference_matches(&p, &id(4)).unwrap());
    p.clips[0].start_us = TimeUs(42);
    p.clips[0].duration_us = TimeUs(10_000);
    p.characters[0].subtitle_style.font_size = 20.;
    assert!(audio_reference_matches(&p, &id(4)).unwrap());
    let mut changed = p.clone();
    let Voice::Voicevox { settings, .. } = &mut changed.characters[0].voice else {
        panic!("VOICEVOX fixture required");
    };
    settings.speed_scale = 1.5;
    assert!(!audio_reference_matches(&changed, &id(4)).unwrap());
    let ClipContent::Speech { spoken_text, .. } = &mut p.clips[0].content else {
        panic!()
    };
    *spoken_text = Some("別のよみ".into());
    assert!(!audio_reference_matches(&p, &id(4)).unwrap());
}

#[test]
fn parent_binding_roundtrips_and_cannot_be_removed_or_replaced() {
    use sungeki_speech::SpeechJobInput;
    let base =
        SpeechJobInput::new(7, &project(), &id(4), "http://127.0.0.1:55021", &identity()).unwrap();
    let input = base.clone().for_parent(&id(90)).unwrap();
    assert_ne!(base.input_hash(), input.input_hash());
    let restored: SpeechJobInput =
        serde_json::from_slice(&serde_json::to_vec(&input).unwrap()).unwrap();
    assert_eq!(restored.parent_job_id(), Some(id(90).as_str()));
    assert_eq!(restored.input_hash(), input.input_hash());
    assert_eq!(
        input.clone().for_parent(&id(90)).unwrap().input_hash(),
        input.input_hash()
    );
    assert!(input.clone().for_parent(&id(91)).is_err());
    assert!(base.for_parent("../bad").is_err());
    for parent in [serde_json::Value::Null, serde_json::json!(id(91))] {
        let mut json = serde_json::to_value(&input).unwrap();
        json["parent_job_id"] = parent;
        assert!(serde_json::from_value::<SpeechJobInput>(json).is_err());
    }
}
