use sungeki_editor::{Command, History};
use sungeki_project::*;
#[test]
fn sbv2_voice_survives_save_roundtrip_and_undo_without_claiming_synthesis() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    let recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    p.characters.push(recipe.roles[0].character.clone());
    let mut character = p.characters[0].clone();
    character.voice = Voice::Sbv2 {
        settings: Sbv2Settings {
            model_id: 0,
            speaker_id: 0,
            language: Sbv2Language::JP,
            style: "Neutral".into(),
            style_weight: 1.,
            length: 1.,
            sdp_ratio: 0.2,
            noise: 0.6,
            noisew: 0.8,
            auto_split: true,
            split_interval: 0.5,
        },
        identity: Sbv2Identity {
            engine_revision: "fixture-only".into(),
            model_sha256: "a".repeat(64),
            config_sha256: "b".repeat(64),
            style_vectors_sha256: "c".repeat(64),
            text_encoder_fingerprint: "d".repeat(64),
        },
    };
    let before = p.clone();
    let mut history = History::new(p).unwrap();
    history
        .apply(
            0,
            &[Command::SetCharacter {
                character: character.clone(),
            }],
        )
        .unwrap();
    p = serde_json::from_slice(&serde_json::to_vec(history.project()).unwrap()).unwrap();
    validate(&p).unwrap();
    assert_eq!(p.characters[0], character);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &before);
    if let Voice::Sbv2 { identity, .. } = &mut character.voice {
        identity.model_sha256 = "invalid".into();
    }
    assert!(
        history
            .apply(2, &[Command::SetCharacter { character }])
            .is_err()
    );
    assert_eq!(history.project(), &before);
}
