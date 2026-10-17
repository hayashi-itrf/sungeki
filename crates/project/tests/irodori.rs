use sungeki_project::{IrodoriIdentity, IrodoriSettings};

fn settings() -> IrodoriSettings {
    IrodoriSettings {
        voice_id: "registered-japanese".into(),
        caption: Some("落ち着いた声".into()),
        cfg_scale: 3.,
        num_steps: 40,
        seed: 42,
    }
}
#[test]
fn explicit_inputs_roundtrip_without_random_or_server_defaults() {
    let original = settings();
    original.validate().unwrap();
    let encoded = serde_json::to_value(&original).unwrap();
    assert_eq!(encoded["seed"], 42);
    assert_eq!(
        serde_json::from_value::<IrodoriSettings>(encoded.clone()).unwrap(),
        original
    );
    for field in ["seed", "num_steps", "cfg_scale", "voice_id"] {
        let mut missing = encoded.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<IrodoriSettings>(missing).is_err());
    }
    for field in ["ref_audio_path", "voice", "checkpoint"] {
        let mut extra = encoded.clone();
        extra[field] = "unexpected".into();
        assert!(serde_json::from_value::<IrodoriSettings>(extra).is_err());
    }
    let mut random = encoded;
    random["seed"] = (-1).into();
    assert!(serde_json::from_value::<IrodoriSettings>(random).is_err());
}
#[test]
fn invalid_generation_inputs_are_rejected() {
    for scale in [f64::NAN, f64::INFINITY, -0.1, 30.1] {
        let mut value = settings();
        value.cfg_scale = scale;
        assert!(value.validate().is_err());
    }
    for steps in [0, 201] {
        let mut value = settings();
        value.num_steps = steps;
        assert!(value.validate().is_err());
    }
    for caption in [" ".into(), "voice\0caption".into(), "あ".repeat(1366)] {
        let mut value = settings();
        value.caption = Some(caption);
        assert!(value.validate().is_err());
    }
    let mut value = settings();
    value.voice_id = "voice\n".into();
    assert!(value.validate().is_err());
}
#[test]
fn codec_and_voice_originals_are_required_independently() {
    let value = IrodoriIdentity {
        checkpoint: "local-model".into(),
        model_manifest_sha256: "a".repeat(64),
        codec_manifest_sha256: "b".repeat(64),
        engine_manifest_sha256: "c".repeat(64),
        voice_manifest_sha256: "d".repeat(64),
        sample_rate: 48000,
        channels: 1,
    };
    value.validate().unwrap();
    let mut changed = value.clone();
    changed.codec_manifest_sha256 = "B".repeat(64);
    assert!(changed.validate().is_err());
    let mut changed = value.clone();
    changed.voice_manifest_sha256.clear();
    assert!(changed.validate().is_err());
    let mut changed = value.clone();
    changed.sample_rate = 0;
    assert!(changed.validate().is_err());
    let mut changed = value;
    changed.channels = 3;
    assert!(changed.validate().is_err());
}
