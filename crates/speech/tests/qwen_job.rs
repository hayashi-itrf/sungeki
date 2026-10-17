use serde_json::json;
use sungeki_project::{ClipContent, Project, QwenIdentity, QwenSettings, Voice};
use sungeki_speech::qwen::{Catalog, QwenJobInput};
fn input() -> QwenJobInput {
    let mut project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/psd-speech-project.json"
    ))
    .unwrap();
    let clip = project
        .clips
        .iter()
        .find(|c| matches!(c.content, ClipContent::Speech { .. }))
        .unwrap();
    let id = clip.id.clone();
    let ClipContent::Speech { character_id, .. } = &clip.content else {
        unreachable!()
    };
    let char_id = character_id.clone();
    let identity = QwenIdentity {
        backend: "official".into(),
        model_id: "/ssd/qwen".into(),
        model_manifest_sha256: "a".repeat(64),
        engine_manifest_sha256: "b".repeat(64),
        voice_manifest_sha256: "c".repeat(64),
        sample_rate: 24000,
        channels: 1,
    };
    project
        .characters
        .iter_mut()
        .find(|c| c.id == char_id)
        .unwrap()
        .voice = Voice::Qwen {
        settings: QwenSettings {
            model: "qwen3-tts".into(),
            voice: "Vivian".into(),
            speed: 1.,
            language: "Japanese".into(),
            instruct: None,
        },
        identity: identity.clone(),
    };
    let catalog=Catalog::parse(json!({"status":"healthy","backend":{"name":"official","model_id":"/ssd/qwen","ready":true},"version":"test"}),json!({"data":[{"id":"qwen3-tts"}]}),json!({"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]})).unwrap();
    QwenJobInput::new(
        7,
        &project,
        &id,
        "http://127.0.0.1:8880",
        &identity,
        &catalog,
    )
    .unwrap()
}
#[test]
fn durable_roundtrip_restores_exact_prepared_input_and_rejects_tampering() {
    let input = input();
    let bytes = serde_json::to_vec(&input).unwrap();
    let restored: QwenJobInput = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.input_hash(), input.input_hash());
    assert_eq!(
        restored.prepared().unwrap().audio().fingerprint(),
        input.prepared().unwrap().audio().fingerprint()
    );
    assert_eq!(restored.endpoint(), "http://127.0.0.1:8880/");
    assert!(restored.matches_project(input.project()).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for (pointer, new) in [
        ("/input_hash", json!("0".repeat(64))),
        ("/capture/source_revision", json!(8)),
        ("/capture/format_version", json!(2)),
        ("/capture/lip_sync_version", json!(999)),
        ("/capture/endpoint", json!("http://192.168.0.1:8880/")),
        (
            "/capture/identity/voice_manifest_sha256",
            json!("d".repeat(64)),
        ),
        ("/capture/catalog/0/backend/model_id", json!("other")),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = new;
        assert!(
            serde_json::from_value::<QwenJobInput>(changed).is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn parent_and_profile_are_immutable_and_input_hash_tracks_them() {
    let original = input();
    let parent = "12345678-1234-1234-1234-123456789abc";
    let selected = original
        .clone()
        .for_parent(parent)
        .unwrap()
        .for_profile("qwen-local")
        .unwrap();
    assert_ne!(selected.input_hash(), original.input_hash());
    assert_eq!(selected.parent_job_id(), Some(parent));
    assert_eq!(selected.profile_id(), Some("qwen-local"));
    assert_eq!(
        selected
            .clone()
            .for_parent(parent)
            .unwrap()
            .for_profile("qwen-local")
            .unwrap()
            .input_hash(),
        selected.input_hash()
    );
    assert!(selected.clone().for_profile("other").is_err());
    assert!(
        selected
            .for_parent("22345678-1234-1234-1234-123456789abc")
            .is_err()
    );
    for profile in ["", "../profile", "a b"] {
        assert!(original.clone().for_profile(profile).is_err());
    }
    assert!(original.for_parent("invalid").is_err());
}
#[test]
fn captured_project_does_not_adopt_later_text_or_voice_changes() {
    let original = input();
    let mut changed = original.project().clone();
    let clip = changed
        .clips
        .iter_mut()
        .find(|c| c.id == original.clip_id())
        .unwrap();
    if let ClipContent::Speech { spoken_text, .. } = &mut clip.content {
        *spoken_text = Some("別の読み".into());
    }
    assert!(!original.matches_project(&changed).unwrap());
    assert!(original.matches_project(original.project()).unwrap());
}
