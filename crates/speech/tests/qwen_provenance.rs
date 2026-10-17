use serde_json::json;
use sungeki_project::{
    AssetProbe, ClipContent, Project, QwenIdentity, QwenSettings, SpeechAudio, Voice,
};
use sungeki_speech::{
    audio_reference_matches,
    qwen::{Catalog, PreparedQwenClip},
};
#[test]
fn qwen_audio_provenance_roundtrip_and_current_input_checks() {
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
    let asset_id = project
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Audio(_)))
        .unwrap()
        .id
        .clone();
    let audio = SpeechAudio {
        irodori_source: None,
        qwen_source: Some(input.audio().audio_source().unwrap()),
        sbv2_source: None,
        asset_id,
        input_fingerprint: input.audio().fingerprint().into(),
        engine_version: identity.engine_manifest_sha256.clone(),
        lip_sync: None,
    };
    if let ClipContent::Speech { audio: target, .. } = &mut project
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .content
    {
        *target = Some(audio);
    }
    sungeki_project::validate(&project).unwrap();
    assert!(audio_reference_matches(&project, &id).unwrap());
    let saved: Project = serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert!(audio_reference_matches(&saved, &id).unwrap());
    for mode in 0..5 {
        let mut changed = project.clone();
        let clip = changed.clips.iter_mut().find(|c| c.id == id).unwrap();
        let ClipContent::Speech {
            subtitle_text,
            audio: Some(audio),
            ..
        } = &mut clip.content
        else {
            unreachable!()
        };
        match mode {
            0 => *subtitle_text = "変更".into(),
            1 => audio.qwen_source = None,
            2 => {
                audio
                    .qwen_source
                    .as_mut()
                    .unwrap()
                    .selection
                    .metadata_sha256 = "d".repeat(64)
            }
            3 => audio.qwen_source.as_mut().unwrap().identity.sample_rate = 48000,
            _ => {
                audio
                    .qwen_source
                    .as_mut()
                    .unwrap()
                    .identity
                    .voice_manifest_sha256 = "d".repeat(64)
            }
        }
        assert!(
            !audio_reference_matches(&changed, &id).unwrap_or(false),
            "{mode}"
        );
    }
}
