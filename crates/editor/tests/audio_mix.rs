use sungeki_editor::*;
use sungeki_project::*;

fn id(n: u8) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}
fn mix(lufs: f64, peak: f64) -> AudioMix {
    AudioMix {
        normalization: Some(Normalization {
            target_lufs: lufs,
            true_peak_db: peak,
        }),
    }
}
#[test]
fn master_settings_are_explicit_persistent_undoable_and_clearable() {
    let original = Project::empty(id(1), "master".into());
    let mut session = EditSession::new(original.clone()).unwrap();
    session
        .execute(EditRequest {
            operation_id: id(2),
            expected_revision: 0,
            action: EditAction::Apply {
                commands: vec![Command::SetAudioMix {
                    audio_mix: mix(-18.0, -1.0),
                }],
            },
        })
        .unwrap();
    let edited = session.history().project().clone();
    assert_eq!(edited.audio_mix, mix(-18.0, -1.0));
    let mut expected = original.clone();
    expected.audio_mix = mix(-18.0, -1.0);
    assert_eq!(edited, expected);
    assert_eq!(
        serde_json::from_str::<Project>(&serde_json::to_string(&edited).unwrap()).unwrap(),
        edited
    );
    let mut session = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    session
        .execute(EditRequest {
            operation_id: id(3),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(2),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &original);
    session
        .execute(EditRequest {
            operation_id: id(4),
            expected_revision: 2,
            action: EditAction::Redo {
                expected_operation_id: id(2),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &edited);
    assert_eq!(
        apply_batch(
            &edited,
            &[Command::SetAudioMix {
                audio_mix: AudioMix {
                    normalization: None
                }
            }]
        )
        .unwrap(),
        original
    );
}
#[test]
fn invalid_master_targets_reject_the_whole_batch() {
    let original = Project::empty(id(1), "master".into());
    for (lufs, peak) in [
        (f64::NAN, -1.0),
        (-18.0, f64::INFINITY),
        (-70.1, -1.0),
        (0.1, -1.0),
        (-18.0, -20.1),
        (-18.0, 0.1),
    ] {
        assert!(
            apply_batch(
                &original,
                &[
                    Command::RenameProject {
                        title: "changed".into()
                    },
                    Command::SetAudioMix {
                        audio_mix: mix(lufs, peak)
                    }
                ]
            )
            .is_err()
        );
    }
}
