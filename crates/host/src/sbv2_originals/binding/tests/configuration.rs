use super::startup::request;
use crate::{
    sbv2_originals::configuration::{self, Profile},
    speech_journal::decode_input,
};
use serde_json::json;
use std::{fs, sync::atomic::AtomicBool};
use sungeki_protocol::ErrorCode;

fn document(request: &crate::sbv2_startup::Request) -> serde_json::Value {
    json!({"format_version":1,"profiles":[{"id":"local-jp","name":"日本語の音声",
        "binding":request.binding,"cpu_only":true,"catalog":request.catalog.models()}]})
}
#[test]
fn configured_profile_prepares_same_measured_input_without_spawning() {
    let (fixture, request, pid) = request("speech");
    let mut sessions = fixture.open();
    let (first, _) = super::job::jobs(&mut sessions, &request);
    let stored = sessions.store.load_job(&first).unwrap().unwrap();
    let (input, _) = decode_input(&stored).unwrap();
    let config = document(&request);
    let path = fixture.0.join("sbv2.json");
    let database = fixture.0.join("recovery.sqlite");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    let profile = configuration::load(&database, "local-jp", &AtomicBool::new(false)).unwrap();
    assert_eq!(profile.name, "日本語の音声");
    assert!(profile.cpu_only);
    let prepared = profile
        .prepare(
            input.source_revision(),
            input.project(),
            input.clip_id(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(prepared.input_hash(), input.input_hash());
    let captured = profile
        .capture(
            input.source_revision(),
            input.project(),
            input.clip_id(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(captured.input_hash(), prepared.input_hash());
    crate::build_record::profiles::tests::check_prepared_child(
        crate::build_record::profiles::LoadedProfile::Sbv2(profile.clone()),
        &input,
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!pid.exists());
    fs::write(
        &request.binding.originals.model,
        b"changed after configuration",
    )
    .unwrap();
    assert_eq!(
        profile
            .prepare(
                input.source_revision(),
                input.project(),
                input.clip_id(),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .code,
        ErrorCode::ExternalModification
    );
    assert!(!pid.exists());
    // Public start can capture an expected selection without reading weights.
    // Full prepare and the actual worker must still reject changed originals.
    let captured = profile
        .capture(
            input.source_revision(),
            input.project(),
            input.clip_id(),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(captured.input_hash(), prepared.input_hash());
    fs::write(&request.binding.originals.config, b"{}").unwrap();
    assert_eq!(
        profile
            .capture(
                input.source_revision(),
                input.project(),
                input.clip_id(),
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .code,
        ErrorCode::ExternalModification
    );
}
fn failure(result: Result<Profile, sungeki_protocol::ApiError>) -> sungeki_protocol::ApiError {
    match result {
        Ok(_) => panic!("unexpected valid configuration"),
        Err(error) => error,
    }
}
#[test]
fn absent_invalid_ambiguous_or_unsupported_configuration_is_actionable() {
    let (fixture, request, _) = request("speech");
    let database = fixture.0.join("recovery.sqlite");
    let path = fixture.0.join("sbv2.json");
    let missing = failure(configuration::load(
        &database,
        "local-jp",
        &AtomicBool::new(false),
    ));
    assert_eq!(missing.code, ErrorCode::CapabilityUnavailable);
    assert_eq!(
        missing.details["configuration_path"],
        path.to_str().unwrap()
    );
    assert!(!missing.suggested_actions.is_empty());
    let original = document(&request);
    for variation in 0..8 {
        let mut value = original.clone();
        match variation {
            0 => value["format_version"] = json!(2),
            1 => value["unknown"] = json!(true),
            2 => value["profiles"][0]["unknown"] = json!(true),
            3 => value["profiles"] = json!([]),
            4 => value["profiles"]
                .as_array_mut()
                .unwrap()
                .push(original["profiles"][0].clone()),
            5 => value["profiles"][0]["id"] = json!("../outside"),
            6 => value["profiles"][0]["binding"]["endpoint"] = json!("http://example.com:5050"),
            _ => value["profiles"][0]["catalog"] = json!({"0":{}}),
        }
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            failure(configuration::load(
                &database,
                "local-jp",
                &AtomicBool::new(false)
            ))
            .code,
            ErrorCode::ValidationFailed,
            "variation {variation}"
        );
    }
    fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(
        failure(configuration::load(
            &database,
            "missing",
            &AtomicBool::new(false)
        ))
        .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        failure(configuration::load(
            &database,
            "../outside",
            &AtomicBool::new(false)
        ))
        .code,
        ErrorCode::ValidationFailed
    );
    assert_eq!(
        failure(configuration::load(
            &database,
            "local-jp",
            &AtomicBool::new(true)
        ))
        .code,
        ErrorCode::Cancelled
    );
    assert_eq!(
        failure(configuration::load(
            std::path::Path::new("relative.sqlite"),
            "local-jp",
            &AtomicBool::new(false)
        ))
        .code,
        ErrorCode::ValidationFailed
    );
}
#[test]
fn linked_nonregular_and_oversized_configuration_is_not_read() {
    let (fixture, request, _) = request("speech");
    let database = fixture.0.join("recovery.sqlite");
    let path = fixture.0.join("sbv2.json");
    let original = fixture.0.join("other.json");
    fs::write(&original, serde_json::to_vec(&document(&request)).unwrap()).unwrap();
    std::os::unix::fs::symlink(&original, &path).unwrap();
    assert!(configuration::load(&database, "local-jp", &AtomicBool::new(false)).is_err());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(configuration::load(&database, "local-jp", &AtomicBool::new(false)).is_err());
    fs::remove_dir(&path).unwrap();
    fs::File::create(&path)
        .unwrap()
        .set_len(32 * 1024 * 1024 + 1)
        .unwrap();
    assert!(configuration::load(&database, "local-jp", &AtomicBool::new(false)).is_err());
    fs::remove_file(&path).unwrap();
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    assert!(configuration::load(&database, "local-jp", &AtomicBool::new(false)).is_err());
}

#[test]
fn profile_listing_returns_only_labels_without_inspecting_models_or_starting_engine() {
    let (fixture, request, pid) = request("speech");
    fs::write(
        fixture.0.join("sbv2.json"),
        serde_json::to_vec(&document(&request)).unwrap(),
    )
    .unwrap();
    fs::remove_file(&request.binding.originals.model).unwrap();
    let sessions = fixture.open();
    let shared = std::sync::Arc::new(std::sync::Mutex::new(sessions));
    let response = crate::sessions::Sessions::execute(
        &shared,
        sungeki_protocol::SessionRequest::SpeechJob {
            request: sungeki_protocol::SpeechJobRequest::ListSbv2Profiles,
        },
    )
    .unwrap();
    let sungeki_protocol::SessionResponse::Sbv2Profiles { profiles } = response else {
        panic!()
    };
    assert_eq!(
        serde_json::to_value(profiles).unwrap(),
        json!([{"id":"local-jp","name":"日本語の音声","cpu_only":true}])
    );
    assert!(!pid.exists());
}

#[test]
fn runtime_manifest_above_four_mib_loads_without_truncation() {
    let (fixture, request, _) = request("speech");
    let mut value = document(&request);
    let files = value["profiles"][0]["binding"]["engine"]["files"]
        .as_object_mut()
        .unwrap();
    for index in 0..45000 {
        files.insert(
            format!("runtime/dependency_{index:05}"),
            json!(format!(
                "/configured/runtime/site-packages/{}/module.py",
                "path".repeat(20)
            )),
        );
    }
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(bytes.len() > 4 * 1024 * 1024);
    fs::write(fixture.0.join("sbv2.json"), bytes).unwrap();
    let profile = configuration::load(
        &fixture.0.join("recovery.sqlite"),
        "local-jp",
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        profile
            .binding
            .engine
            .files
            .contains_key("runtime/dependency_44999")
    );
    // Listing/reading configuration does not claim these synthetic paths exist.
}
