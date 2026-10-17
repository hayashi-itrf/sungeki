mod sbv2_target_support;
use sbv2_target_support::{environment, id, project};
use serde_json::json;
use sungeki_speech::sbv2::Sbv2JobInput;

fn input() -> Sbv2JobInput {
    let (catalog, _, identity) = environment();
    let config = serde_json::to_string(&json!({"version":"2.7.0-JP-Extra",
        "data":{"sampling_rate":44100,"spk2id":{"声":0},"num_styles":1,
        "style2id":{"Neutral":0}}}))
    .unwrap();
    Sbv2JobInput::new(
        7,
        &project(&identity),
        &id(4),
        "http://127.0.0.1:5050",
        &identity,
        &catalog,
        &config,
    )
    .unwrap()
}
#[test]
fn durable_roundtrip_preserves_input_and_parent_binding() {
    let base = input();
    let bound = base.clone().for_parent(&id(90)).unwrap();
    assert_ne!(base.input_hash(), bound.input_hash());
    let restored: Sbv2JobInput =
        serde_json::from_slice(&serde_json::to_vec(&bound).unwrap()).unwrap();
    assert_eq!(restored.input_hash(), bound.input_hash());
    assert_eq!(restored.source_revision(), 7);
    assert_eq!(restored.endpoint(), "http://127.0.0.1:5050/");
    assert_eq!(restored.clip_id(), id(4));
    assert_eq!(restored.project(), bound.project());
    assert_eq!(restored.identity(), bound.identity());
    assert_eq!(restored.lip_sync_settings(), bound.lip_sync_settings());
    assert_eq!(restored.parent_job_id(), Some(id(90).as_str()));
    assert_eq!(restored.prepared().unwrap().audio().resolved().text, "よみ");
    assert_eq!(
        bound.clone().for_parent(&id(90)).unwrap().input_hash(),
        bound.input_hash()
    );
    assert!(bound.for_parent(&id(91)).is_err());
    assert!(base.for_parent("../other").is_err());
}
#[test]
fn durable_load_rejects_modified_capture_and_unknown_fields() {
    let original = serde_json::to_value(input().for_parent(&id(90)).unwrap()).unwrap();
    for (pointer, value) in [
        ("/capture/format_version", json!(2)),
        ("/capture/source_revision", json!(8)),
        ("/capture/lip_sync_version", json!(999)),
        ("/capture/lip_sync_settings/window_us", json!(999)),
        ("/capture/project/title", json!("changed")),
        ("/capture/clip_id", json!(id(99))),
        ("/capture/endpoint", json!("http://127.0.0.1:5051/")),
        ("/capture/endpoint", json!("http://127.0.0.1:5050")),
        ("/capture/endpoint", json!("http://example.com/")),
        ("/capture/identity/model_sha256", json!("d".repeat(64))),
        ("/capture/catalog/0/model_path", json!("/other/model")),
        ("/capture/model_config", json!("{}")),
        ("/capture/parent_job_id", json!(null)),
        ("/capture/parent_job_id", json!(id(91))),
        ("/input_hash", json!("a".repeat(64))),
    ] {
        let mut altered = original.clone();
        *altered.pointer_mut(pointer).unwrap() = value;
        assert!(
            serde_json::from_value::<Sbv2JobInput>(altered).is_err(),
            "{pointer}"
        );
    }
    for pointer in ["", "/capture"] {
        let mut altered = original.clone();
        altered.pointer_mut(pointer).unwrap()["extra"] = json!(true);
        assert!(serde_json::from_value::<Sbv2JobInput>(altered).is_err());
    }
}

#[test]
fn profile_id_is_immutable_and_part_of_durable_input_hash() {
    let original = input();
    let bound = original.clone().for_profile("local-jp").unwrap();
    assert_ne!(original.input_hash(), bound.input_hash());
    assert_eq!(bound.profile_id(), Some("local-jp"));
    assert_eq!(
        bound.clone().for_profile("local-jp").unwrap().input_hash(),
        bound.input_hash()
    );
    assert!(bound.clone().for_profile("other").is_err());
    let encoded = serde_json::to_value(&bound).unwrap();
    let restored: Sbv2JobInput = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(restored.input_hash(), bound.input_hash());
    for id in [json!(null), json!("other"), json!("../outside")] {
        let mut edited = encoded.clone();
        edited["capture"]["profile_id"] = id;
        assert!(serde_json::from_value::<Sbv2JobInput>(edited).is_err());
    }
    assert!(original.for_profile("../outside").is_err());
}
