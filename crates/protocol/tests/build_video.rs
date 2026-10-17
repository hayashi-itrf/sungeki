use sungeki_protocol::*;
#[test]
fn start_wait_is_optional_and_requires_minor_136_when_supplied() {
    let recipe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    let mut value =
        serde_json::json!({"job_id":"job", "recipe":recipe, "project_path":"/tmp/new.vvm.json"});
    let args: StartBuildVideoArgs = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(args.wait_ms, None);
    for wait in [0, 30000] {
        value["wait_ms"] = wait.into();
        let request = SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start {
                args: serde_json::from_value(value.clone()).unwrap(),
            },
        };
        assert_eq!(request.minimum_protocol_minor(), 136);
    }
    value["wait_ms"] = (-1).into();
    assert!(serde_json::from_value::<StartBuildVideoArgs>(value).is_err());
}
#[test]
fn production_commands_require_their_wire_version_and_reject_unknown_fields() {
    for (value, minor) in [
        (serde_json::json!({"type":"get","job_id":"id"}), 135),
        (
            serde_json::json!({"type":"get","job_id":"id","wait_ms":30000}),
            135,
        ),
        (serde_json::json!({"type":"cancel","job_id":"id"}), 73),
        (
            serde_json::json!({"type":"list","after":null,"limit":20}),
            73,
        ),
    ] {
        let request = SessionRequest::BuildVideo {
            request: serde_json::from_value(value).unwrap(),
        };
        assert_eq!(request.minimum_protocol_minor(), minor);
        let bytes = serde_json::to_vec(&request).unwrap();
        assert!(serde_json::from_slice::<SessionRequest>(&bytes).is_ok());
    }
    assert!(
        serde_json::from_value::<BuildVideoRequest>(
            serde_json::json!({"type":"get","job_id":"id","restart":true})
        )
        .is_err()
    );
}
