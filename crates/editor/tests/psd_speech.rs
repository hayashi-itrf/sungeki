use sungeki_editor::*;
use sungeki_project::*;
#[test]
fn binding_roundtrip_undo_and_invalid_reference_are_atomic() {
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    validate(&p).unwrap();
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&p).unwrap()).unwrap(),
        p
    );
    let mut history = History::new(p.clone()).unwrap();
    history
        .apply(
            0,
            &[Command::SetPsdSpeech {
                clip_id: p.clips[0].id.clone(),
                speech: None,
            }],
        )
        .unwrap();
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
    let ClipContent::Psd {
        speech: Some(binding),
        ..
    } = &p.clips[0].content
    else {
        panic!()
    };
    for kind in 0..3 {
        let mut bad = binding.clone();
        match kind {
            0 => bad.character_id = "missing".into(),
            1 => bad.open_option = bad.closed_option.clone(),
            _ => bad.group = "/missing".into(),
        }
        assert!(
            history
                .apply(
                    2,
                    &[
                        Command::RenameProject {
                            title: "discard".into()
                        },
                        Command::SetPsdSpeech {
                            clip_id: p.clips[0].id.clone(),
                            speech: Some(bad)
                        }
                    ]
                )
                .is_err()
        );
        assert_eq!(history.project(), &p);
    }
    assert!(
        history
            .apply(
                2,
                &[Command::RemoveCharacter {
                    character_id: binding.character_id.clone()
                }]
            )
            .is_err()
    );
}
