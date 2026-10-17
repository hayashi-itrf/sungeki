use sungeki_protocol::{SessionRequest, SpeechJobRequest};

#[test]
fn named_profile_tool_dispatches_to_shared_host_api() {
    let mut context = crate::context::Context::default();
    let args = serde_json::json!({"session_id":"session", "job_id":"job",
        "expected_revision":2,"clip_id":"clip","profile_id":"local-jp"});
    let (request, _) =
        crate::dispatch::request("start_sbv2_speech_job", args.clone(), &mut context).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 68);
    assert!(
        matches!(request, SessionRequest::SpeechJob { request: SpeechJobRequest::StartSbv2 { args } } if args.profile_id == "local-jp")
    );
    let mut invalid = args;
    invalid["command"] = serde_json::json!("not accepted");
    assert!(crate::dispatch::request("start_sbv2_speech_job", invalid, &mut context).is_err());
    assert!(
        crate::tools::definitions()
            .iter()
            .any(|tool| tool.name == "start_sbv2_speech_job")
    );
}

#[test]
fn profile_listing_is_read_only_and_rejects_arbitrary_paths() {
    let mut context = crate::context::Context::default();
    let (request, _) =
        crate::dispatch::request("list_sbv2_profiles", serde_json::json!({}), &mut context)
            .unwrap();
    assert!(matches!(
        request,
        SessionRequest::SpeechJob {
            request: SpeechJobRequest::ListSbv2Profiles
        }
    ));
    assert_eq!(request.minimum_protocol_minor(), 69);
    assert!(
        crate::dispatch::request(
            "list_sbv2_profiles",
            serde_json::json!({"path":"/other"}),
            &mut context
        )
        .is_err()
    );
}

#[test]
fn inspected_profile_details_use_profile_id_and_shared_host() {
    let mut context = crate::context::Context::default();
    let (request, _) = crate::dispatch::request(
        "get_sbv2_profile",
        serde_json::json!({"profile_id":"local-jp"}),
        &mut context,
    )
    .unwrap();
    assert_eq!(request.minimum_protocol_minor(), 70);
    assert!(matches!(
        request,
        SessionRequest::SpeechJob {
            request: SpeechJobRequest::GetSbv2Profile { .. }
        }
    ));
    assert!(
        crate::tools::definitions()
            .iter()
            .any(|tool| tool.name == "get_sbv2_profile")
    );
    assert!(
        crate::dispatch::request(
            "get_sbv2_profile",
            serde_json::json!({"profile_id":"local-jp","model_path":"/other"}),
            &mut context
        )
        .is_err()
    );
}

#[test]
fn qwen_profile_tool_dispatches_to_shared_host_api() {
    let mut context = crate::context::Context::default();
    let args = serde_json::json!({"session_id":"session", "job_id":"job",
        "expected_revision":2,"clip_id":"clip","profile_id":"local-jp"});
    let (request, _) =
        crate::dispatch::request("start_qwen_speech_job", args.clone(), &mut context).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 101);
    assert!(
        matches!(request, SessionRequest::SpeechJob { request: SpeechJobRequest::StartQwen { args } } if args.profile_id == "local-jp")
    );
    let mut invalid = args;
    invalid["command"] = serde_json::json!("not accepted");
    assert!(crate::dispatch::request("start_qwen_speech_job", invalid, &mut context).is_err());
    assert!(
        crate::tools::definitions()
            .iter()
            .any(|tool| tool.name == "start_qwen_speech_job")
    );
}

#[test]
fn qwen_profile_discovery_rejects_arbitrary_paths() {
    let mut context = crate::context::Context::default();
    for (name, args) in [
        ("list_qwen_profiles", serde_json::json!({})),
        (
            "get_qwen_profile",
            serde_json::json!({"profile_id":"local"}),
        ),
    ] {
        let (request, _) = crate::dispatch::request(name, args.clone(), &mut context).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 102);
        assert!(
            crate::tools::definitions()
                .iter()
                .any(|tool| tool.name == name)
        );
        let mut bad = args;
        bad["path"] = serde_json::json!("/arbitrary/settings.json");
        assert!(crate::dispatch::request(name, bad, &mut context).is_err());
    }
}

#[test]
fn irodori_profile_tool_dispatches_to_shared_host_api() {
    let mut context = crate::context::Context::default();
    let args = serde_json::json!({"session_id":"session", "job_id":"job",
        "expected_revision":2,"clip_id":"clip","profile_id":"local-jp"});
    let (request, _) =
        crate::dispatch::request("start_irodori_speech_job", args.clone(), &mut context).unwrap();
    assert_eq!(request.minimum_protocol_minor(), 105);
    assert!(
        matches!(request, SessionRequest::SpeechJob { request: SpeechJobRequest::StartIrodori { args } } if args.profile_id == "local-jp")
    );
    let mut invalid = args;
    invalid["command"] = serde_json::json!("not accepted");
    assert!(crate::dispatch::request("start_irodori_speech_job", invalid, &mut context).is_err());
    assert!(
        crate::tools::definitions()
            .iter()
            .any(|tool| tool.name == "start_irodori_speech_job")
    );
}

#[test]
fn irodori_profile_discovery_rejects_arbitrary_paths() {
    let mut context = crate::context::Context::default();
    for (name, args) in [
        ("list_irodori_profiles", serde_json::json!({})),
        (
            "get_irodori_profile",
            serde_json::json!({"profile_id":"local"}),
        ),
    ] {
        let (request, _) = crate::dispatch::request(name, args.clone(), &mut context).unwrap();
        assert_eq!(request.minimum_protocol_minor(), 105);
        assert!(
            crate::tools::definitions()
                .iter()
                .any(|tool| tool.name == name)
        );
        let mut bad = args;
        bad["path"] = serde_json::json!("/arbitrary/settings.json");
        assert!(crate::dispatch::request(name, bad, &mut context).is_err());
    }
}
