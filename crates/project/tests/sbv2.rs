use serde_json::json;
use sungeki_project::{Sbv2Language, Sbv2Settings};
fn fixture() -> serde_json::Value {
    json!({"model_id":0,"speaker_id":0,"language":"JP","style":"Neutral",
        "style_weight":1.0,"length":1.0,"sdp_ratio":0.2,"noise":0.6,"noisew":0.8,
        "auto_split":true,"split_interval":0.5})
}
#[test]
fn settings_round_trip_without_implicit_fields_or_unknown_options() {
    let value: Sbv2Settings = serde_json::from_value(fixture()).unwrap();
    value.validate().unwrap();
    assert_eq!(value.language, Sbv2Language::JP);
    assert_eq!(
        serde_json::from_value::<Sbv2Settings>(serde_json::to_value(&value).unwrap()).unwrap(),
        value
    );
    let mut missing = fixture();
    missing.as_object_mut().unwrap().remove("length");
    assert!(serde_json::from_value::<Sbv2Settings>(missing).is_err());
    let mut unknown = fixture();
    unknown["legacy_speed"] = json!(1);
    assert!(serde_json::from_value::<Sbv2Settings>(unknown).is_err());
    let mut language = fixture();
    language["language"] = json!("Japanese");
    assert!(serde_json::from_value::<Sbv2Settings>(language).is_err());
}
#[test]
fn shared_validation_rejects_each_invalid_control() {
    for (key, bad) in [
        ("style", json!("\n")),
        ("style_weight", json!(-1)),
        ("length", json!(0)),
        ("sdp_ratio", json!(1.1)),
        ("noise", json!(-1)),
        ("noisew", json!(-1)),
        ("split_interval", json!(61)),
    ] {
        let mut data = fixture();
        data[key] = bad;
        assert!(
            serde_json::from_value::<Sbv2Settings>(data)
                .unwrap()
                .validate()
                .is_err(),
            "{key}"
        );
    }
    let mut data: Sbv2Settings = serde_json::from_value(fixture()).unwrap();
    data.noise = f64::NAN;
    assert!(data.validate().is_err());
}

#[test]
fn identity_requires_all_original_hashes_and_bounded_revision() {
    use sungeki_project::Sbv2Identity;
    let identity = Sbv2Identity {
        engine_revision: "fixture".into(),
        model_sha256: "a".repeat(64),
        config_sha256: "b".repeat(64),
        style_vectors_sha256: "c".repeat(64),
        text_encoder_fingerprint: "d".repeat(64),
    };
    identity.validate().unwrap();
    let json = serde_json::to_value(&identity).unwrap();
    assert_eq!(
        serde_json::from_value::<Sbv2Identity>(json.clone()).unwrap(),
        identity
    );
    for name in [
        "model_sha256",
        "config_sha256",
        "style_vectors_sha256",
        "text_encoder_fingerprint",
    ] {
        for invalid in ["", "ABCDEF", &"A".repeat(64)] {
            let mut value = json.clone();
            value[name] = serde_json::json!(invalid);
            assert!(
                serde_json::from_value::<Sbv2Identity>(value)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
    }
    let mut changed = identity;
    changed.engine_revision = "revision\n".into();
    assert!(changed.validate().is_err());
}
