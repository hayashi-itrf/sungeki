use sungeki_editor::{Command, History};
use sungeki_project::*;
#[test]
fn qwen_voice_survives_save_roundtrip_and_undo_without_claiming_synthesis() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    let recipe: BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    p.characters.push(recipe.roles[0].character.clone());
    let mut character = p.characters[0].clone();
    character.voice = Voice::Qwen {
        settings: QwenSettings {
            model: "qwen3-tts".into(),
            voice: "Vivian".into(),
            language: "Japanese".into(),
            speed: 1.,
            instruct: Some("穏やかに".into()),
        },
        identity: QwenIdentity {
            backend: "official".into(),
            model_id: "/ssd/qwen".into(),
            model_manifest_sha256: "a".repeat(64),
            engine_manifest_sha256: "b".repeat(64),
            voice_manifest_sha256: "c".repeat(64),
            sample_rate: 24000,
            channels: 1,
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
    if let Voice::Qwen { identity, .. } = &mut character.voice {
        identity.model_manifest_sha256 = "invalid".into();
    }
    assert!(
        history
            .apply(2, &[Command::SetCharacter { character }])
            .is_err()
    );
    assert_eq!(history.project(), &before);
}
