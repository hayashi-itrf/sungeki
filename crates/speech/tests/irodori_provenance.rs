#[path = "irodori/common.rs"]
mod common;
use sungeki_project::{AssetProbe, ClipContent, Project, SpeechAudio, Voice};
use sungeki_speech::{audio_reference_matches, irodori::PreparedIrodoriInput};
#[test]
fn irodori_audio_provenance_roundtrip_and_current_input_checks() {
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
    let source = common::source();
    project
        .characters
        .iter_mut()
        .find(|c| c.id == char_id)
        .unwrap()
        .voice = Voice::Irodori {
        settings: source.settings.clone(),
        identity: source.identity.clone(),
    };
    let clip = project.clips.iter().find(|c| c.id == id).unwrap();
    let ClipContent::Speech {
        subtitle_text,
        spoken_text,
        ..
    } = &clip.content
    else {
        panic!()
    };
    let input = PreparedIrodoriInput::new(
        subtitle_text,
        spoken_text.as_deref(),
        &project.pronunciation_dictionary,
        &source,
    )
    .unwrap();
    let asset_id = project
        .assets
        .iter()
        .find(|a| matches!(a.probe, AssetProbe::Audio(_)))
        .unwrap()
        .id
        .clone();
    let audio = SpeechAudio {
        irodori_source: Some(input.audio_source().clone()),
        qwen_source: None,
        sbv2_source: None,
        asset_id,
        input_fingerprint: input.fingerprint().into(),
        engine_version: source.identity.engine_manifest_sha256.clone(),
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
    let mut positioned = project.clone();
    positioned
        .clips
        .iter_mut()
        .find(|c| c.id == id)
        .unwrap()
        .transform
        .x += 10.;
    assert!(audio_reference_matches(&positioned, &id).unwrap());
    for mode in 0..3 {
        let mut changed = project.clone();
        let character = changed
            .characters
            .iter_mut()
            .find(|c| c.id == char_id)
            .unwrap();
        let Voice::Irodori { settings, identity } = &mut character.voice else {
            panic!()
        };
        match mode {
            0 => settings.seed += 1,
            1 => identity.codec_manifest_sha256 = "f".repeat(64),
            _ => changed
                .pronunciation_dictionary
                .push(sungeki_project::PronunciationEntry {
                    written: "追加語".into(),
                    spoken: "ついかご".into(),
                }),
        }
        assert!(!audio_reference_matches(&changed, &id).unwrap());
    }
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
            1 => audio.irodori_source = None,
            2 => audio.irodori_source.as_mut().unwrap().metadata_sha256 = "d".repeat(64),
            3 => audio.irodori_source.as_mut().unwrap().identity.sample_rate = 48000,
            _ => {
                audio
                    .irodori_source
                    .as_mut()
                    .unwrap()
                    .identity
                    .voice_manifest_sha256 = "f".repeat(64)
            }
        }
        assert!(
            !audio_reference_matches(&changed, &id).unwrap_or(false),
            "{mode}"
        );
    }
}
