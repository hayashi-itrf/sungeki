use super::{job::seed_qwen, request};
use crate::{
    qwen_originals::configuration::{Profile, load, read_candidate},
    speech_journal_tests::Fixture,
};
use serde_json::json;
use std::{fs, sync::atomic::AtomicBool};
use sungeki_protocol::ErrorCode;

#[test]
fn profile_roundtrip_prepares_bound_input_and_rejects_changed_original() {
    let (engine, request, pid) = request("speech");
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (_, input) = seed_qwen(&mut sessions, &request);
    let profile = Profile {
        id: "qwen-local".into(),
        name: "ローカルQwen".into(),
        binding: request.binding,
        port: request.port,
        identity: request.identity.clone(),
        default_settings: request.settings,
        catalog: [
            json!({"status":"healthy","backend":{"name":"official","model_id":request.identity.model_id,"ready":true},"version":"test"}),
            json!({"data":[{"id":"qwen3-tts"}]}),
            json!({"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]}),
        ],
    };
    let database = engine.0.join("host.sqlite");
    let path = engine.0.join("qwen.json");
    let flag = AtomicBool::new(false);
    assert_eq!(
        load(&database, &profile.id, &flag).err().unwrap().code,
        ErrorCode::CapabilityUnavailable
    );
    let mut setup = serde_json::to_value(&profile).unwrap();
    setup.as_object_mut().unwrap().remove("identity");
    setup["format_version"] = json!(1);
    setup["sample_rate"] = json!(24000);
    setup["channels"] = json!(1);
    setup["binding"]["originals"]["model"] = json!({});
    let setup_path = engine.0.join("setup.json");
    fs::write(&setup_path, serde_json::to_vec(&setup).unwrap()).unwrap();
    let candidate = super::super::super::setup::prepare(&setup_path, &flag).unwrap();
    assert_eq!(
        candidate["profiles"][0]["identity"],
        serde_json::to_value(&profile.identity).unwrap()
    );
    assert!(
        !path.exists(),
        "candidate generation must not install configuration"
    );
    assert!(
        !pid.exists(),
        "candidate generation must not start an engine"
    );
    let value = json!({"format_version":1,"profiles":[profile]});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let report = super::super::super::preflight::check(&path, &flag).unwrap();
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(report["synthesis_checked"], false);
    assert_eq!(report["dependency_completeness_checked"], false);
    assert_eq!(report["profiles"][0]["profile_id"], "qwen-local");
    let loaded = load(&database, "qwen-local", &flag).unwrap();
    let prepared = loaded
        .prepare(2, input.project(), input.clip_id(), &flag)
        .unwrap();
    assert_eq!(prepared.profile_id(), Some("qwen-local"));
    crate::build_record::profiles::tests::check_prepared_child(
        crate::build_record::profiles::LoadedProfile::Qwen(loaded.clone()),
        &crate::speech_journal::Input::Qwen(input.clone()),
    );
    assert_eq!(
        prepared.input_hash(),
        input
            .clone()
            .for_profile("qwen-local")
            .unwrap()
            .input_hash()
    );
    assert!(
        !pid.exists(),
        "profile loading/preparation must never start an engine"
    );
    fs::write(engine.0.join("model/config.json"), "changed").unwrap();
    assert!(super::super::super::preflight::check(&path, &flag).is_err());
    assert_eq!(
        loaded
            .prepare(2, input.project(), input.clip_id(), &flag)
            .err()
            .unwrap()
            .code,
        ErrorCode::ExternalModification
    );
    assert_eq!(
        load(&database, "other", &flag).err().unwrap().code,
        ErrorCode::NotFound
    );
    for case in 0..6 {
        let mut bad = value.clone();
        match case {
            0 => bad["format_version"] = json!(2),
            1 => bad["profiles"] = json!([value["profiles"][0], value["profiles"][0]]),
            2 => bad["profiles"][0]["id"] = json!("../escape"),
            3 => bad["profiles"][0]["port"] = json!(80),
            4 => bad["profiles"][0]["identity"]["model_id"] = json!("wrong"),
            _ => bad["profiles"][0]["binding"]["unexpected"] = json!(true),
        }
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(read_candidate(&path, &flag).is_err(), "case {case}");
    }
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(engine.0.join("model/config.json"), &path).unwrap();
    assert!(read_candidate(&path, &flag).is_err());
}
