use super::*;
use serde_json::{Value, json};
fn fixture() -> Value {
    json!({"format_version":1,"profiles":[{
        "id":"local","name":"Irodori","port":50123,
        "binding":{"engine_root":"/fixture/engine","checkpoint":"/fixture/model",
            "codec":"/fixture/codec","registry":"/fixture/registry","work_root":"/fixture/work",
            "executable":"/fixture/venv/bin/python","executable_target":"/fixture/python",
            "launcher":"/fixture/launch.py","device":"cpu","precision":"fp32",
            "originals":{"model":{},"codec":{},"engine":{},"voice":{}}},
        "identity":{"checkpoint":"model","model_manifest_sha256":"a".repeat(64),
            "codec_manifest_sha256":"b".repeat(64),"engine_manifest_sha256":"c".repeat(64),
            "voice_manifest_sha256":"d".repeat(64),"sample_rate":48000,"channels":1},
        "default_settings":{"voice_id":"registered","caption":null,"cfg_scale":3.,"num_steps":40,"seed":42},
        "model":{"checkpoint":"model","speaker_conditioning":true,"caption_conditioning":false,
            "sample_rate":48000,"channels":1},
        "voices":[{"id":"registered","name":"登録話者","reference":"none","caption":null}]
    }]})
}
fn parsed(value: &Value) -> Result<Vec<Profile>, ApiError> {
    parse(&serde_json::to_vec(value).unwrap())
}
#[test]
fn metadata_validation_does_not_start_or_require_a_runtime() {
    let profiles = parsed(&fixture()).unwrap();
    assert_eq!(profiles[0].id, "local");
    assert_eq!(profiles[0].default_settings.seed, 42);
    // File binding is a separate blocking stage, not a claim of readiness.
    assert!(
        profiles[0]
            .checked_catalog(&AtomicBool::new(false))
            .is_err()
    );
}
#[test]
fn invalid_profile_defaults_and_model_metadata_rejected() {
    for mode in 0..9 {
        let mut value = fixture();
        let p = &mut value["profiles"][0];
        match mode {
            0 => p["id"] = "../outside".into(),
            1 => p["port"] = 80.into(),
            2 => p["default_settings"]["voice_id"] = "missing".into(),
            3 => p["model"]["checkpoint"] = "other".into(),
            4 => p["model"]["sample_rate"] = 24000.into(),
            5 => p["default_settings"]["caption"] = "未対応".into(),
            6 => p["identity"]["codec_manifest_sha256"] = "invalid".into(),
            7 => p["legacy_config"] = true.into(),
            _ => {
                p["model"]["channels"] = 2.into();
                p["identity"]["channels"] = 2.into();
            }
        }
        assert!(parsed(&value).is_err(), "{mode}");
    }
    let mut duplicate = fixture();
    let p = duplicate["profiles"][0].clone();
    duplicate["profiles"].as_array_mut().unwrap().push(p);
    assert!(parsed(&duplicate).is_err());
    let mut version = fixture();
    version["format_version"] = 2.into();
    assert!(parsed(&version).is_err());
}
#[test]
fn registry_catalog_must_match_expected_metadata() {
    let profile = parsed(&fixture()).unwrap().remove(0);
    profile.match_registry(profile.voices.clone()).unwrap();
    let mut changed = profile.voices.clone();
    changed[0].name = "変更後".into();
    assert_eq!(
        profile.match_registry(changed).unwrap_err().code,
        ErrorCode::ExternalModification
    );
    let mut changed = profile.voices.clone();
    changed[0].reference = sungeki_speech::irodori::ReferenceKind::Audio;
    assert!(profile.match_registry(changed).is_err());
}
#[test]
fn missing_next_configuration_is_explicit() {
    let root = std::env::temp_dir().join(format!("irodori-missing-{}", uuid::Uuid::new_v4()));
    let error = read_profiles(&root.join("host.db"), &AtomicBool::new(false))
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
    assert!(error.details["configuration_path"].ends_with("irodori.json"));
    assert_eq!(
        load(&root.join("host.db"), "../bad", &AtomicBool::new(false))
            .err()
            .unwrap()
            .code,
        ErrorCode::ValidationFailed
    );
}
