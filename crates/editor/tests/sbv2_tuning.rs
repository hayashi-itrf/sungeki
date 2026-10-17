use sungeki_editor::{Command, History, apply_batch};
use sungeki_project::*;
#[path = "common/speech.rs"]
mod setup;
use setup::*;

#[test]
fn tuning_clears_only_affected_audio_and_undo_restores_original_references() {
    let mut p = apply_batch(&Project::empty(id(1), "SBV2 tuning".into()), &commands()).unwrap();
    let settings: Sbv2Settings = serde_json::from_value(serde_json::json!({
        "model_id": 2, "speaker_id": 4, "language": "JP", "style": "Happy",
        "style_weight": 1, "length": 1, "sdp_ratio": 0.2, "noise": 0.6,
        "noisew": 0.8, "auto_split": true, "split_interval": 0.5
    }))
    .unwrap();
    let identity = Sbv2Identity {
        engine_revision: "fixture-only".into(),
        model_sha256: "a".repeat(64),
        config_sha256: "b".repeat(64),
        style_vectors_sha256: "c".repeat(64),
        text_encoder_fingerprint: "d".repeat(64),
    };
    p.characters[0].voice = Voice::Sbv2 { settings, identity };
    let mut other = p.characters[0].clone();
    other.id = id(8);
    p.characters.push(other);
    p.assets.push(Asset {
        id: id(6),
        name: "fixture audio".into(),
        source: None,
        credit: "fixture".into(),
        locator: AssetLocator::Absolute {
            path: "/missing/fixture.wav".into(),
        },
        content_hash: Some("a".repeat(64)),
        probe: AssetProbe::Audio(AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(10_000),
            sample_rate: 24000,
            channels: 1,
            frames: 240,
            byte_length: 524,
        }),
    });
    // This reference tests editor history, not synthesis or audio freshness.
    if let ClipContent::Speech { audio, .. } = &mut p.clips[0].content {
        *audio = Some(SpeechAudio {
            irodori_source: None,
            qwen_source: None,
            sbv2_source: None,
            asset_id: id(6),
            input_fingerprint: "b".repeat(64),
            engine_version: "fixture-only".into(),
            lip_sync: None,
        });
    }
    let mut other_clip = p.clips[0].clone();
    other_clip.id = id(9);
    if let ClipContent::Speech { character_id, .. } = &mut other_clip.content {
        *character_id = id(8);
    }
    p.clips.push(other_clip);
    let mut changed = p.characters[0].clone();
    if let Voice::Sbv2 { settings, .. } = &mut changed.voice {
        settings.length = 1.5;
        settings.style_weight = 5.;
        settings.auto_split = false;
    }
    let command = Command::SetCharacter { character: changed };
    let mut history = History::new(p.clone()).unwrap();
    history.apply(0, &[command]).unwrap();
    let after = history.project().clone();
    assert!(matches!(
        after.clips[0].content,
        ClipContent::Speech { audio: None, .. }
    ));
    assert_eq!(after.clips[1], p.clips[1]);
    assert_eq!(after.assets, p.assets);
    assert_eq!(after.clips[0].duration_us, p.clips[0].duration_us);
    assert_eq!(
        serde_json::from_slice::<Project>(&serde_json::to_vec(&after).unwrap()).unwrap(),
        after
    );
    history.undo(1).unwrap();
    assert_eq!(history.project(), &p);
    history.redo(2).unwrap();
    assert_eq!(history.project(), &after);
}
