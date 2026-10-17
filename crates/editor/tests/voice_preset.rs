use sungeki_editor::*;
use sungeki_project::*;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn fixture() -> Project {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap()
}
fn command(p: &Project, change: bool) -> Command {
    let mut voice = p.characters[0].voice.clone();
    if change {
        let Voice::Voicevox { settings, .. } = &mut voice else {
            panic!()
        };
        settings.speed_scale = 1.5;
    }
    Command::ApplyVoicePreset {
        preset_id: id(8000),
        preset_revision: 1,
        preset: Preset {
            schema_version: 1,
            name: "別の声".into(),
            description: String::new(),
            tags: vec![],
            content: PresetContent::Voice {
                voice,
                credit: "VOICEVOX:検証".into(),
            },
        },
        character_id: p.characters[0].id.clone(),
        credit: "立ち絵:検証\nVOICEVOX:検証".into(),
    }
}
#[test]
fn changed_voice_clears_only_target_audio_and_replay_and_undo_preserve_everything() {
    let mut p = fixture();
    let mut other = p.characters[0].clone();
    other.id = id(8001);
    p.characters.push(other);
    let mut clip = p
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap()
        .clone();
    clip.id = id(8002);
    if let ClipContent::Speech {
        character_id,
        audio,
        ..
    } = &mut clip.content
    {
        assert!(audio.is_some());
        *character_id = id(8001);
    }
    p.clips.push(clip.clone());
    let mut session = EditSession::new(p.clone()).unwrap();
    let request = EditRequest {
        operation_id: id(8003),
        expected_revision: 0,
        action: EditAction::Apply {
            commands: vec![command(&p, true)],
        },
    };
    let receipt = session.execute(request.clone()).unwrap();
    assert_eq!(session.execute(request).unwrap(), receipt);
    let changed = session.history().project();
    assert_eq!(changed.assets, p.assets);
    assert_eq!(changed.characters[0].name, p.characters[0].name);
    assert_eq!(
        changed.characters[0].subtitle_style,
        p.characters[0].subtitle_style
    );
    assert_eq!(changed.characters[1], p.characters[1]);
    for c in &changed.clips {
        if let ClipContent::Speech {
            character_id,
            audio,
            ..
        } = &c.content
        {
            if character_id == &p.characters[0].id {
                assert!(audio.is_none());
            } else {
                assert_eq!(c, &clip);
            }
        } else {
            assert_eq!(Some(c), p.clips.iter().find(|old| old.id == c.id));
        }
    }
    session
        .execute(EditRequest {
            operation_id: id(8004),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: id(8003),
            },
        })
        .unwrap();
    assert_eq!(session.history().project(), &p);
}
#[test]
fn same_voice_keeps_audio_and_bad_credit_or_missing_character_rejects_atomic_batch() {
    let p = fixture();
    let next = apply_batch(&p, &[command(&p, false)]).unwrap();
    assert_eq!(next.clips, p.clips);
    for case in 0..4 {
        let mut bad = command(&p, true);
        let Command::ApplyVoicePreset {
            credit,
            character_id,
            preset_revision,
            preset_id,
            ..
        } = &mut bad
        else {
            panic!()
        };
        match case {
            0 => *credit = "別の表記".into(),
            1 => *character_id = id(9999),
            2 => *preset_revision = 0,
            _ => *preset_id = "bad".into(),
        }
        let mut history = History::new(p.clone()).unwrap();
        assert!(
            history
                .apply(
                    0,
                    &[
                        Command::RenameProject {
                            title: "変更しない".into()
                        },
                        bad
                    ]
                )
                .is_err()
        );
        assert_eq!(history.project(), &p);
    }
}
