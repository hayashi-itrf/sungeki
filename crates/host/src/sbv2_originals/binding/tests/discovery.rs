use super::startup::request;
use crate::sbv2_originals::{configuration::Profile, discovery::details};
use serde_json::json;
use std::sync::atomic::AtomicBool;
use sungeki_project::Sbv2Language;
use sungeki_protocol::*;
fn profile(request: &crate::sbv2_startup::Request) -> Profile {
    Profile {
        id: "local-jp".into(),
        name: "日本語の声".into(),
        binding: request.binding.clone(),
        cpu_only: true,
        catalog: serde_json::to_value(request.catalog.models()).unwrap(),
    }
}
#[test]
fn discovery_measures_declared_model_and_engine_without_running_them() {
    let (fixture, request, pid) = request("speech");
    let profile = profile(&request);
    std::fs::write(
        fixture.0.join("sbv2.json"),
        serde_json::to_vec(&json!({"format_version":1,"profiles":[profile]})).unwrap(),
    )
    .unwrap();
    let sessions = fixture.open();
    let shared = std::sync::Arc::new(std::sync::Mutex::new(sessions));
    let response = crate::sessions::Sessions::execute(
        &shared,
        SessionRequest::SpeechJob {
            request: SpeechJobRequest::GetSbv2Profile {
                args: GetSbv2ProfileArgs {
                    profile_id: "local-jp".into(),
                },
            },
        },
    )
    .unwrap();
    let SessionResponse::Sbv2Profile { details } = response else {
        panic!()
    };
    assert_eq!(details.identity, request.identity);
    assert_eq!(details.default_settings, request.settings);
    assert_eq!(details.speakers[0].name, "voice");
    assert_eq!(details.speakers[0].id, 0);
    assert_eq!(details.styles, vec!["Neutral"]);
    assert_eq!(details.languages, vec![Sbv2Language::JP]);
    assert!(
        !serde_json::to_string(&details)
            .unwrap()
            .contains(fixture.0.to_str().unwrap())
    );
    assert!(!pid.exists());
}
#[test]
fn changed_originals_change_identity_and_missing_or_mismatched_config_is_rejected() {
    let (_fixture, request, pid) = request("speech");
    let profile = profile(&request);
    let before = details(&profile, &AtomicBool::new(false)).unwrap();
    std::fs::write(&profile.binding.originals.model, b"different model").unwrap();
    let after = details(&profile, &AtomicBool::new(false)).unwrap();
    assert_ne!(before.identity.model_sha256, after.identity.model_sha256);
    assert_eq!(
        before.identity.engine_revision,
        after.identity.engine_revision
    );
    assert_eq!(
        details(&profile, &AtomicBool::new(true)).unwrap_err().code,
        ErrorCode::Cancelled
    );
    let config = std::fs::read_to_string(&profile.binding.originals.config).unwrap();
    std::fs::write(
        &profile.binding.originals.config,
        config.replace("voice", "other"),
    )
    .unwrap();
    assert!(details(&profile, &AtomicBool::new(false)).is_err());
    std::fs::remove_file(&profile.binding.originals.config).unwrap();
    assert!(details(&profile, &AtomicBool::new(false)).is_err());
    assert!(!pid.exists());
}
#[test]
fn ambiguous_or_unrelated_metadata_never_selects_a_different_original() {
    let (_fixture, request, pid) = request("speech");
    let mut profile = profile(&request);
    profile.catalog["1"] = profile.catalog["0"].clone();
    assert!(details(&profile, &AtomicBool::new(false)).is_err());
    profile.catalog["1"]["model_path"] = json!("/unconfigured/model");
    let selected = details(&profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(selected.default_settings.model_id, 0);
    profile.catalog["0"]["model_path"] = json!("/another/unconfigured/model");
    assert!(details(&profile, &AtomicBool::new(false)).is_err());
    assert!(!pid.exists());
}

#[test]
fn non_extra_model_reports_existing_style_and_permitted_languages() {
    let (_fixture, request, pid) = request("speech");
    let mut profile = profile(&request);
    let config = std::fs::read_to_string(&profile.binding.originals.config).unwrap();
    std::fs::write(
        &profile.binding.originals.config,
        config
            .replace("2.7.0-JP-Extra", "2.7.0")
            .replace("Neutral", "Excited"),
    )
    .unwrap();
    profile.catalog["0"]["style2id"] = json!({"Excited":0});
    let selected = details(&profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(selected.default_settings.style, "Excited");
    assert_eq!(selected.styles, vec!["Excited"]);
    assert_eq!(
        selected.languages,
        vec![Sbv2Language::JP, Sbv2Language::EN, Sbv2Language::ZH]
    );
    assert!(!pid.exists());
}
