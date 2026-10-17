use super::startup::request;
use crate::sbv2_originals::preflight;
use serde_json::json;
use std::{fs, sync::atomic::AtomicBool};

#[test]
fn candidate_check_measures_all_profiles_without_installing_or_launching() {
    let (fixture, request, pid) = request("speech");
    let profile = json!({"id":"local-jp","name":"検査用", "binding":request.binding,
        "cpu_only":true,"catalog":request.catalog.models()});
    let path = fixture.0.join("candidate.json");
    let mut second = profile.clone();
    second["id"] = json!("second");
    let bytes =
        serde_json::to_vec(&json!({"format_version":1,"profiles":[profile,second]})).unwrap();
    fs::write(&path, &bytes).unwrap();
    let result = preflight::check(&path, &AtomicBool::new(false)).unwrap();
    let result = serde_json::to_value(result).unwrap();
    assert_eq!(result["check"], "declared_originals");
    assert_eq!(result["synthesis_checked"], false);
    assert_eq!(result["dependency_completeness_checked"], false);
    assert_eq!(result["profiles"].as_array().unwrap().len(), 2);
    assert_eq!(
        result["profiles"][0]["identity"],
        serde_json::to_value(&request.identity).unwrap()
    );
    assert!(!pid.exists());
    assert!(!fixture.0.join("sbv2.json").exists());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(preflight::check(&path, &AtomicBool::new(true)).is_err());
    fs::remove_file(&request.binding.originals.model).unwrap();
    assert!(preflight::check(&path, &AtomicBool::new(false)).is_err());
}

#[test]
fn candidate_check_rejects_invalid_configuration_and_leaf_links() {
    let (fixture, _, pid) = request("speech");
    let path = fixture.0.join("candidate.json");
    fs::write(&path, b"{\"format_version\":1,\"profiles\":[]}").unwrap();
    assert!(preflight::check(&path, &AtomicBool::new(false)).is_err());
    let link = fixture.0.join("link.json");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(preflight::check(&link, &AtomicBool::new(false)).is_err());
    assert!(!pid.exists());
}
