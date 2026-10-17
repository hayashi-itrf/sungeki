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
fn audio(p: &Project) -> Option<&SpeechAudio> {
    let ClipContent::Speech { audio, .. } = &p.clips[0].content else {
        panic!()
    };
    audio.as_ref()
}
#[test]
fn reference_roundtrips_and_clearing_is_undoable_without_removing_original() {
    let p = fixture();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&p).unwrap()).unwrap(),
        p
    );
    let mut history = History::new(p.clone()).unwrap();
    history
        .apply(
            0,
            &[Command::SetSpeechAudio {
                clip_id: id(4),
                audio: None,
            }],
        )
        .unwrap();
    assert!(audio(history.project()).is_none());
    assert_eq!(history.project().assets, p.assets);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
}
#[test]
fn relevant_edits_clear_audio_and_undo_restores_it() {
    let p = fixture();
    let mut changed_voice = character();
    let Voice::Voicevox { settings, .. } = &mut changed_voice.voice else {
        panic!("VOICEVOX fixture required");
    };
    settings.speed_scale = 1.5;
    for command in [
        Command::SetSpeech {
            clip_id: id(4),
            character_id: id(2),
            subtitle_text: "変更した字幕".into(),
            spoken_text: Some("よみ".into()),
        },
        Command::SetCharacter {
            character: changed_voice,
        },
        Command::SetPronunciationDictionary {
            entries: vec![PronunciationEntry {
                written: "字幕".into(),
                spoken: "じまく".into(),
            }],
        },
    ] {
        let mut history = History::new(p.clone()).unwrap();
        history.apply(0, &[command]).unwrap();
        assert!(audio(history.project()).is_none());
        assert_eq!(history.project().assets, p.assets);
        history.undo(1).unwrap();
        assert_eq!(history.project(), &p);
    }
}
#[test]
fn same_input_and_cosmetic_changes_keep_audio() {
    let p = fixture();
    let mut renamed = character();
    renamed.name = "名前だけ変更".into();
    renamed.subtitle_style.font_size = 70.;
    renamed.subtitle_style.layout = Some(SubtitleLayout {
        alignment: TextAlignment::Center,
        wrap: true,
    });
    renamed.subtitle_style.outline = Some(SubtitleOutline {
        width: 3,
        color: "#000000".into(),
    });
    let next = apply_batch(
        &p,
        &[
            Command::SetSpeech {
                clip_id: id(4),
                character_id: id(2),
                subtitle_text: "字幕".into(),
                spoken_text: Some("よみ".into()),
            },
            Command::SetPronunciationDictionary { entries: vec![] },
            Command::SetCharacter { character: renamed },
            Command::MoveClip {
                clip_id: id(4),
                start_us: TimeUs(1_000_000),
            },
        ],
    )
    .unwrap();
    assert_eq!(audio(&next), audio(&p));
}
#[test]
fn dangling_or_invalid_references_reject_the_batch() {
    let p = fixture();
    assert!(apply_batch(&p, &[Command::RemoveAsset { asset_id: id(6) }]).is_err());
    for case in 0..3 {
        let mut reference = reference();
        match case {
            0 => reference.asset_id = id(99),
            1 => reference.input_fingerprint = "bad".into(),
            _ => reference.engine_version = "".into(),
        }
        assert!(
            apply_batch(
                &p,
                &[Command::SetSpeechAudio {
                    clip_id: id(4),
                    audio: Some(reference)
                }]
            )
            .is_err()
        );
    }
    let removed = apply_batch(
        &p,
        &[
            Command::SetSpeechAudio {
                clip_id: id(4),
                audio: None,
            },
            Command::RemoveAsset { asset_id: id(6) },
        ],
    )
    .unwrap();
    assert!(removed.assets.is_empty());
}

#[test]
fn gain_is_non_destructive_undoable_and_survives_speech_replacement() {
    let p = fixture();
    let mut history = History::new(p.clone()).unwrap();
    history
        .apply(
            0,
            &[Command::SetSpeechGain {
                clip_id: id(4),
                gain_db: Some(-6.0),
            }],
        )
        .unwrap();
    let changed = history.project().clone();
    assert_eq!(audio(&changed), audio(&p));
    assert_eq!(changed.assets, p.assets);
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&changed).unwrap()).unwrap(),
        changed
    );
    let replaced = apply_batch(
        &changed,
        &[Command::SetSpeech {
            clip_id: id(4),
            character_id: id(2),
            subtitle_text: "別の発話".into(),
            spoken_text: None,
        }],
    )
    .unwrap();
    assert!(matches!(
        replaced.clips[0].content,
        ClipContent::Speech {
            gain_db: Some(-6.0),
            audio: None,
            ..
        }
    ));
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &changed);
    let reset = apply_batch(
        &changed,
        &[Command::SetSpeechGain {
            clip_id: id(4),
            gain_db: None,
        }],
    )
    .unwrap();
    assert_eq!(reset, p);
}
#[test]
fn invalid_gain_rolls_back_the_entire_batch() {
    let p = fixture();
    for gain in [f64::NAN, f64::INFINITY, -60.01, 24.01] {
        assert!(
            apply_batch(
                &p,
                &[
                    Command::RenameProject {
                        title: "must not persist".into()
                    },
                    Command::SetSpeechGain {
                        clip_id: id(4),
                        gain_db: Some(gain)
                    },
                ]
            )
            .is_err()
        );
    }
    for gain in [-60.0, 0.0, 24.0] {
        let adjusted = apply_batch(
            &p,
            &[Command::SetSpeechGain {
                clip_id: id(4),
                gain_db: Some(gain),
            }],
        )
        .unwrap();
        assert_eq!(audio(&adjusted), audio(&p));
        let duplicated = apply_batch(
            &adjusted,
            &[Command::DuplicateClip {
                clip_id: id(4),
                new_clip_id: id(98),
                start_us: TimeUs(3_000_000),
            }],
        )
        .unwrap();
        assert_eq!(duplicated.clips[0].content, duplicated.clips[1].content);
    }
    let mut visual = p.clone();
    visual.clips[0].content = ClipContent::Shape {
        shape: ShapeKind::Rectangle,
        fill: "#ffffff".into(),
    };
    visual.tracks[0].kind = TrackKind::Visual;
    assert!(
        apply_batch(
            &visual,
            &[Command::SetSpeechGain {
                clip_id: id(4),
                gain_db: Some(0.0)
            }]
        )
        .is_err()
    );
    assert!(
        apply_batch(
            &p,
            &[Command::SetSpeechGain {
                clip_id: id(99),
                gain_db: None
            }]
        )
        .is_err()
    );
}
