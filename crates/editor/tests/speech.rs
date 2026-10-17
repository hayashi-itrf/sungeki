use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod fixture;
use fixture::*;
fn request(n: u32, revision: u32, action: EditAction) -> EditRequest {
    EditRequest {
        operation_id: id(n),
        expected_revision: revision,
        action,
    }
}
#[test]
fn speech_creation_roundtrips_restores_and_undoes_atomically() {
    let original = Project::empty(id(1), "音声編集".into());
    let mut session = EditSession::new(original.clone()).unwrap();
    let command = request(
        10,
        0,
        EditAction::Apply {
            commands: commands(),
        },
    );
    let receipt = session.execute(command.clone()).unwrap();
    let project = session.history().project().clone();
    assert_eq!(project.clips[0].scene_id, Some(id(5)));
    let serialized = serde_json::to_vec(&project).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&serialized).unwrap(),
        project
    );
    let mut session = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(session.execute(command).unwrap(), receipt);
    session
        .execute(request(
            11,
            1,
            EditAction::Undo {
                expected_operation_id: id(10),
            },
        ))
        .unwrap();
    assert_eq!(session.history().project(), &original);
    session
        .execute(request(
            12,
            2,
            EditAction::Redo {
                expected_operation_id: id(10),
            },
        ))
        .unwrap();
    assert_eq!(session.history().project(), &project);
}
#[test]
fn reference_track_and_reading_failures_reject_the_entire_batch() {
    let original = apply_batch(&Project::empty(id(1), "元の作品".into()), &commands()).unwrap();
    let invalid = [
        Command::RemoveCharacter {
            character_id: id(2),
        },
        Command::SetSpeech {
            clip_id: id(4),
            character_id: id(99),
            subtitle_text: "字幕".into(),
            spoken_text: None,
        },
        Command::SetSpeech {
            clip_id: id(4),
            character_id: id(2),
            subtitle_text: "字幕".into(),
            spoken_text: Some(" ".into()),
        },
    ];
    for command in invalid {
        assert!(
            apply_batch(
                &original,
                &[
                    Command::RenameProject {
                        title: "変更されない".into()
                    },
                    command
                ]
            )
            .is_err()
        );
        assert_eq!(original.title, "元の作品");
    }
    let mut wrong_track = original.clone();
    wrong_track.tracks[0].kind = TrackKind::Visual;
    assert!(validate(&wrong_track).is_err());
    let mut invalid_voice = character();
    let Voice::Voicevox { settings, .. } = &mut invalid_voice.voice else {
        panic!("VOICEVOX fixture required");
    };
    settings.speed_scale = f64::NAN;
    assert!(
        apply_batch(
            &original,
            &[Command::SetCharacter {
                character: invalid_voice
            }]
        )
        .is_err()
    );
    let removed = apply_batch(
        &original,
        &[
            Command::RemoveCharacter {
                character_id: id(2),
            },
            Command::DeleteClip { clip_id: id(4) },
        ],
    )
    .unwrap();
    assert!(removed.clips.is_empty() && removed.characters.is_empty());
}
#[test]
fn changing_reading_keeps_timing_placement_and_scene_and_can_clear_override() {
    let original = apply_batch(&Project::empty(id(1), "発話".into()), &commands()).unwrap();
    let changed = apply_batch(
        &original,
        &[Command::SetSpeech {
            clip_id: id(4),
            character_id: id(2),
            subtitle_text: "新しい字幕".into(),
            spoken_text: None,
        }],
    )
    .unwrap();
    let mut expected = original.clips[0].clone();
    expected.content = ClipContent::Speech {
        gain_db: None,
        audio: None,
        character_id: id(2),
        subtitle_text: "新しい字幕".into(),
        spoken_text: None,
    };
    assert_eq!(changed.clips[0], expected);
    let mut renamed = character();
    renamed.name = "新しい名前".into();
    let changed = apply_batch(
        &changed,
        &[Command::SetCharacter {
            character: renamed.clone(),
        }],
    )
    .unwrap();
    assert_eq!(changed.characters, vec![renamed]);
}
