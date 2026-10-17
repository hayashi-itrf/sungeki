use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod fixture;
use fixture::id;
fn playback() -> AudioPlayback {
    AudioPlayback {
        volume_points: vec![],
        source_start_us: TimeUs(100_000),
        source_duration_us: TimeUs(500_000),
        looped: true,
        gain_db: -18.0,
        fade_in_us: TimeUs(100_000),
        fade_out_us: TimeUs(200_000),
        ducking: None,
    }
}
fn project() -> Project {
    let mut project =
        apply_batch(&Project::empty(id(1), "BGM".into()), &fixture::commands()).unwrap();
    project.assets.push(Asset {
        id: id(6),
        name: "音声原本".into(),
        source: None,
        credit: "test".into(),
        content_hash: Some("a".repeat(64)),
        locator: AssetLocator::Absolute {
            path: "/test.wav".into(),
        },
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(1_000_000),
            sample_rate: 48000,
            channels: 1,
            frames: 48000,
            byte_length: 96044,
        }),
    });
    project.clips[0].content = ClipContent::Audio {
        asset_id: id(6),
        playback: playback(),
    };
    validate(&project).unwrap();
    project
}
#[test]
fn audio_edit_restore_and_history_preserve_selection_and_scene() {
    let original = project();
    let mut session = EditSession::new(original.clone()).unwrap();
    let mut settings = playback();
    settings.gain_db = -12.5;
    settings.ducking = Some(Ducking {
        attenuation_db: -15.0,
        attack_us: TimeUs(250_000),
        release_us: TimeUs(500_000),
    });
    session
        .execute(EditRequest {
            operation_id: id(10),
            expected_revision: 0,
            action: EditAction::Apply {
                commands: vec![Command::SetAudio {
                    clip_id: id(4),
                    asset_id: id(6),
                    playback: settings.clone(),
                }],
            },
        })
        .unwrap();
    let edited = session.history().project().clone();
    assert_eq!(edited.clips[0].scene_id, Some(id(5)));
    assert_eq!(edited.assets, original.assets);
    let encoded = serde_json::to_vec(&edited).unwrap();
    assert_eq!(serde_json::from_slice::<Project>(&encoded).unwrap(), edited);
    let mut session = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    session
        .execute(EditRequest {
            operation_id: id(11),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(10),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &original);
    session
        .execute(EditRequest {
            operation_id: id(12),
            expected_revision: 2,
            action: EditAction::Redo {
                expected_operation_id: id(10),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &edited);
}
#[test]
fn invalid_audio_edits_fail_atomically() {
    let original = project();
    let mut invalid = vec![];
    for change in 0..11 {
        let mut p = playback();
        match change {
            0 => p.source_duration_us = TimeUs(0),
            1 => p.source_start_us = TimeUs(u64::MAX),
            2 => p.source_duration_us = TimeUs(1_000_000),
            3 => p.looped = false,
            4 => p.gain_db = f64::NAN,
            5 => p.gain_db = 25.0,
            6 => p.fade_out_us = TimeUs(2_000_000),
            7 => {
                p.source_start_us = TimeUs(1);
                p.source_duration_us = TimeUs(1);
            }
            _ => {
                p.ducking = Some(Ducking {
                    attenuation_db: if change == 8 { f64::NAN } else { -15.0 },
                    attack_us: TimeUs(if change == 9 { MAX_TIME_US + 1 } else { 0 }),
                    release_us: TimeUs(if change == 10 { u64::MAX } else { 0 }),
                })
            }
        }
        invalid.push(Command::SetAudio {
            clip_id: id(4),
            asset_id: id(6),
            playback: p,
        });
    }
    invalid.extend([
        Command::RemoveAsset { asset_id: id(6) },
        Command::SetAudio {
            clip_id: id(4),
            asset_id: id(99),
            playback: playback(),
        },
    ]);
    for command in invalid {
        assert!(
            apply_batch(
                &original,
                &[
                    Command::RenameProject {
                        title: "失敗時は変更しない".into()
                    },
                    command
                ]
            )
            .is_err()
        );
    }
    let speech = apply_batch(&Project::empty(id(1), "発話".into()), &fixture::commands()).unwrap();
    assert!(
        apply_batch(
            &speech,
            &[Command::SetAudio {
                clip_id: id(4),
                asset_id: id(6),
                playback: playback()
            }]
        )
        .is_err()
    );
}
#[test]
fn unlooped_selection_may_be_trimmed_but_not_extended() {
    let mut p = project();
    p.clips[0].duration_us = TimeUs(500_000);
    let ClipContent::Audio { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.looped = false;
    validate(&p).unwrap();
    p.clips[0].duration_us = TimeUs(500_001);
    assert!(validate(&p).is_err());
    p.clips[0].duration_us = TimeUs(500_000);
    p.tracks[0].kind = TrackKind::Visual;
    assert!(validate(&p).is_err());
}

#[test]
fn volume_points_survive_save_and_history_and_reject_outside_clip() {
    let original = project();
    let mut h = History::new(original.clone()).unwrap();
    let mut settings = playback();
    settings.volume_points = vec![
        VolumePoint {
            at_us: TimeUs(0),
            level: 1.0,
        },
        VolumePoint {
            at_us: original.clips[0].duration_us,
            level: 0.0,
        },
    ];
    h.apply(
        0,
        &[Command::SetAudio {
            clip_id: original.clips[0].id.clone(),
            asset_id: id(6),
            playback: settings.clone(),
        }],
    )
    .unwrap();
    let loaded: Project =
        serde_json::from_slice(&serde_json::to_vec(h.project()).unwrap()).unwrap();
    assert_eq!(&loaded, h.project());
    settings.volume_points[1].at_us.0 += 1;
    assert!(
        h.apply(
            1,
            &[Command::SetAudio {
                clip_id: original.clips[0].id.clone(),
                asset_id: id(6),
                playback: settings
            }]
        )
        .is_err()
    );
    assert_eq!(h.project(), &loaded);
    h.undo(1).unwrap();
    assert_eq!(h.project(), &original);
    h.redo(2).unwrap();
    assert_eq!(h.project(), &loaded);
}
