use sungeki_editor::*;
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;

fn source() -> IrodoriAudioSource {
    IrodoriAudioSource {
        settings: IrodoriSettings {
            voice_id: "registered".into(),
            caption: None,
            cfg_scale: 3.,
            num_steps: 40,
            seed: 42,
        },
        identity: IrodoriIdentity {
            checkpoint: "model".into(),
            model_manifest_sha256: "a".repeat(64),
            codec_manifest_sha256: "b".repeat(64),
            engine_manifest_sha256: "c".repeat(64),
            voice_manifest_sha256: "d".repeat(64),
            sample_rate: 24000,
            channels: 1,
        },
        metadata_sha256: "e".repeat(64),
    }
}
fn fixture() -> Project {
    let mut p = apply_batch(&Project::empty(id(1), "Irodori来歴".into()), &commands()).unwrap();
    let s = source();
    let mut character = character();
    character.voice = Voice::Irodori {
        settings: s.settings.clone(),
        identity: s.identity.clone(),
    };
    p = apply_batch(&p, &[Command::SetCharacter { character }]).unwrap();
    p.assets.push(Asset {
        id: id(6),
        name: "Irodori音声".into(),
        source: None,
        credit: "fixture".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/fixture.wav".into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(10000),
            sample_rate: 24000,
            channels: 1,
            frames: 240,
            byte_length: 524,
        }),
    });
    apply_batch(
        &p,
        &[Command::SetSpeechAudio {
            clip_id: id(4),
            audio: Some(SpeechAudio {
                irodori_source: Some(s),
                qwen_source: None,
                sbv2_source: None,
                asset_id: id(6),
                input_fingerprint: "f".repeat(64),
                engine_version: "c".repeat(64),
                lip_sync: None,
            }),
        }],
    )
    .unwrap()
}
fn audio(p: &Project) -> &SpeechAudio {
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &p.clips[0].content
    else {
        panic!()
    };
    audio
}
#[test]
fn roundtrip_and_seed_edit_preserve_original_and_undo_restores_reference() {
    let p = fixture();
    let restored: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    validate(&restored).unwrap();
    assert_eq!(p, restored);
    let mut character = p.characters[0].clone();
    let Voice::Irodori { settings, .. } = &mut character.voice else {
        panic!()
    };
    settings.seed += 1;
    let mut history = History::new(p.clone()).unwrap();
    history
        .apply(0, &[Command::SetCharacter { character }])
        .unwrap();
    assert!(matches!(
        history.project().clips[0].content,
        ClipContent::Speech { audio: None, .. }
    ));
    assert_eq!(history.project().assets, p.assets);
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
}
#[test]
fn wrong_format_engine_hash_or_metadata_is_rejected_atomically() {
    let p = fixture();
    for mode in 0..3 {
        let mut changed = audio(&p).clone();
        let source = changed.irodori_source.as_mut().unwrap();
        match mode {
            0 => source.identity.sample_rate = 48000,
            1 => source.identity.engine_manifest_sha256 = "d".repeat(64),
            _ => source.metadata_sha256 = "bad".into(),
        }
        let mut history = History::new(p.clone()).unwrap();
        assert!(
            history
                .apply(
                    0,
                    &[Command::SetSpeechAudio {
                        clip_id: id(4),
                        audio: Some(changed)
                    }]
                )
                .is_err()
        );
        assert_eq!(history.project(), &p);
    }
}
#[test]
fn multiple_engine_sources_are_rejected() {
    let p = fixture();
    let mut changed = audio(&p).clone();
    changed.qwen_source = Some(QwenAudioSource {
        selection: QwenSelection {
            backend: "official".into(),
            model_id: "qwen".into(),
            request_model: "qwen".into(),
            voice: "v".into(),
            language: "ja".into(),
            metadata_sha256: "a".repeat(64),
        },
        identity: QwenIdentity {
            backend: "official".into(),
            model_id: "qwen".into(),
            model_manifest_sha256: "a".repeat(64),
            engine_manifest_sha256: "c".repeat(64),
            voice_manifest_sha256: "d".repeat(64),
            sample_rate: 24000,
            channels: 1,
        },
    });
    assert!(
        apply_batch(
            &p,
            &[Command::SetSpeechAudio {
                clip_id: id(4),
                audio: Some(changed)
            }]
        )
        .is_err()
    );
}
