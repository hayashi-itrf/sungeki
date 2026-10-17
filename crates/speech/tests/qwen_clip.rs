use serde_json::json;
use sungeki_project::{ClipContent, Project, QwenIdentity, QwenSettings, Voice};
use sungeki_speech::qwen::{Catalog, PreparedQwenClip};
#[test]
fn project_target_capture_rejects_identity_mismatch_and_stale_changes() {
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
    let input = PreparedQwenClip::new(&project, &id, &catalog, &identity).unwrap();
    assert!(input.matches(&project, &catalog, &identity));
    let mut wrong = identity.clone();
    wrong.voice_manifest_sha256 = "d".repeat(64);
    assert!(PreparedQwenClip::new(&project, &id, &catalog, &wrong).is_err());
    let mut changed = project.clone();
    if let ClipContent::Speech {
        subtitle_text: subtitle,
        ..
    } = &mut changed
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .content
    {
        *subtitle = "変更".into();
    }
    assert!(!input.matches(&changed, &catalog, &identity));
    let mut changed = project.clone();
    changed
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .start_us
        .0 += 1;
    assert!(!input.matches(&changed, &catalog, &identity));
    let mut changed = project;
    let Voice::Qwen { settings, .. } = &mut changed
        .characters
        .iter_mut()
        .find(|c| c.id == char_id)
        .unwrap()
        .voice
    else {
        unreachable!()
    };
    settings.speed = 1.5;
    assert!(!input.matches(&changed, &catalog, &identity));
}
