use super::*;
mod fixture;
use fixture::Fixture;
use serde_json::json;
use sungeki_protocol::ErrorCode;
mod api;
mod configuration;
mod discovery;
mod exchange;
mod job;
mod launch;
mod manifest;
mod preflight;
mod scheduler;
mod startup;
fn setup() -> (Fixture, Binding, Catalog, Sbv2Settings, Sbv2Identity) {
    let fixture = Fixture::new();
    let mut paths = super::super::tests::paths(&fixture.0);
    let styles = fixture.0.join("style_vectors.npy");
    std::fs::rename(&paths.style_vectors, &styles).unwrap();
    paths.style_vectors = styles;
    let measured = inspect(&paths, &AtomicBool::new(false)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    let executable_target = std::fs::canonicalize(&fixture.0)
        .unwrap()
        .join("base-python");
    let bin = fixture.0.join("venv/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("python");
    std::os::unix::fs::symlink(&executable_target, &executable).unwrap();
    std::fs::write(
        fixture.0.join("venv/pyvenv.cfg"),
        b"include-system-site-packages = false\n",
    )
    .unwrap();
    std::fs::write(&executable_target, b"fake interpreter, never executed").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut files = std::collections::BTreeMap::new();
    for name in [
        "server_fastapi.py",
        "config.py",
        "config.yml",
        "next_launcher.py",
    ] {
        let path = fixture.0.join(name);
        std::fs::write(&path, name).unwrap();
        files.insert(name.into(), path);
    }
    let engine = EngineFiles {
        executable,
        executable_target,
        files,
    };
    let identity = Sbv2Identity {
        engine_revision: engine.revision(&AtomicBool::new(false)).unwrap(),
        model_sha256: measured.model_sha256,
        config_sha256: measured.config_sha256,
        style_vectors_sha256: measured.style_vectors_sha256,
        text_encoder_fingerprint: measured.text_encoder_fingerprint,
    };
    let work = fixture.0.parent().unwrap().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let binding = Binding {
        endpoint: "http://127.0.0.1:5050".into(),
        working_directory: fixture.0.clone(),
        work_root: work,
        originals: paths,
        engine,
    };
    let catalog = Catalog::parse(
        json!({"0":{"model_path":"./model","config_path":"config.json",
        "device":"cpu","spk2id":{"voice":0},"id2spk":{"0":"voice"},"style2id":{"Neutral":0}}}),
    )
    .unwrap();
    let settings = serde_json::from_value(json!({"model_id":0,"speaker_id":0,"language":"JP",
        "style":"Neutral","style_weight":1.,"length":1.,"sdp_ratio":0.2,"noise":0.6,
        "noisew":0.8,"auto_split":true,"split_interval":0.5}))
    .unwrap();
    (fixture, binding, catalog, settings, identity)
}
#[test]
fn configured_binding_connects_catalog_paths_config_maps_and_actual_hashes() {
    let (_fixture, binding, catalog, settings, identity) = setup();
    let encoded = serde_json::to_vec(&binding).unwrap();
    let binding: Binding = serde_json::from_slice(&encoded).unwrap();
    binding
        .inspect_selected(
            "http://127.0.0.1:5050/",
            &catalog,
            &settings,
            &identity,
            &AtomicBool::new(false),
        )
        .unwrap();
    let mut info = serde_json::to_value(catalog.models()).unwrap();
    info["0"]["model_path"] = json!(binding.originals.model);
    info["0"]["config_path"] = json!(binding.originals.config);
    binding
        .inspect_selected(
            &binding.endpoint,
            &Catalog::parse(info.clone()).unwrap(),
            &settings,
            &identity,
            &AtomicBool::new(false),
        )
        .unwrap();
    info["0"]["style2id"] = json!({"Neutral":0,"Other":1});
    assert!(
        binding
            .inspect_selected(
                &binding.endpoint,
                &Catalog::parse(info).unwrap(),
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .is_err()
    );
    std::fs::write(&binding.originals.model, b"model-modified").unwrap();
    assert!(
        binding
            .inspect_selected(
                &binding.endpoint,
                &catalog,
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .is_err()
    );
}
#[test]
fn mismatched_server_paths_and_endpoint_are_rejected_before_opening_originals() {
    let (_fixture, binding, catalog, settings, identity) = setup();
    std::fs::remove_file(&binding.originals.model).unwrap();
    assert_eq!(
        binding
            .inspect_selected(
                "http://127.0.0.1:5051",
                &catalog,
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::ExternalModification
    );
    let mut value = serde_json::to_value(catalog.models()).unwrap();
    value["0"]["model_path"] = json!("/unconfigured/model");
    assert_eq!(
        binding
            .inspect_selected(
                &binding.endpoint,
                &Catalog::parse(value).unwrap(),
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::ExternalModification
    );
    for endpoint in [
        "http://example.com/",
        "http://127.0.0.1:5050/private",
        "http://user@127.0.0.1/",
    ] {
        assert!(
            binding
                .inspect_selected(
                    endpoint,
                    &catalog,
                    &settings,
                    &identity,
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
}
#[test]
fn rejects_relative_configuration_wrong_companions_and_unknown_fields() {
    let (_fixture, mut binding, catalog, settings, identity) = setup();
    let mut value = serde_json::to_value(&binding).unwrap();
    value["unknown"] = json!(true);
    assert!(serde_json::from_value::<Binding>(value).is_err());
    binding.originals.style_vectors = binding.working_directory.join("different.npy");
    assert!(
        binding
            .inspect_selected(
                &binding.endpoint,
                &catalog,
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .is_err()
    );
    binding.working_directory = "relative".into();
    assert!(
        binding
            .inspect_selected(
                &binding.endpoint,
                &catalog,
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert!(normalized(Path::new("/../../model")).is_err());
    assert!(normalized(Path::new("/configured/link/../model")).is_err());
}
#[test]
fn engine_code_and_interpreter_changes_invalidate_the_binding() {
    let (_fixture, binding, catalog, settings, identity) = setup();
    let mut files: Vec<_> = binding.engine.files.values().cloned().collect();
    files.push(binding.engine.executable.clone());
    for path in files {
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, vec![b'x'; original.len()]).unwrap();
        assert_eq!(
            binding
                .inspect_selected(
                    &binding.endpoint,
                    &catalog,
                    &settings,
                    &identity,
                    &AtomicBool::new(false)
                )
                .err()
                .unwrap()
                .code,
            ErrorCode::ExternalModification
        );
        std::fs::write(&path, original).unwrap();
    }
    binding
        .inspect_selected(
            &binding.endpoint,
            &catalog,
            &settings,
            &identity,
            &AtomicBool::new(false),
        )
        .unwrap();
}
#[test]
fn incomplete_manifest_wrong_cwd_nonexecutable_and_cancelled_inspection_are_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let (_fixture, binding, catalog, settings, identity) = setup();
    let mut incomplete = binding.engine.clone();
    incomplete.files.remove("config.py");
    assert!(incomplete.revision(&AtomicBool::new(false)).is_err());
    let mut with_empty = binding.engine.clone();
    let empty = binding.working_directory.join("__init__.py");
    std::fs::write(&empty, b"").unwrap();
    with_empty.files.insert("package/__init__.py".into(), empty);
    assert_ne!(
        with_empty.revision(&AtomicBool::new(false)).unwrap(),
        identity.engine_revision
    );
    let mut relocated = binding.clone();
    relocated.engine.files.insert(
        "server_fastapi.py".into(),
        binding.working_directory.join("different.py"),
    );
    assert!(
        relocated
            .inspect_selected(
                &binding.endpoint,
                &catalog,
                &settings,
                &identity,
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert_eq!(
        binding
            .engine
            .revision(&AtomicBool::new(true))
            .err()
            .unwrap()
            .code,
        ErrorCode::Cancelled
    );
    std::fs::set_permissions(
        &binding.engine.executable,
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert!(binding.engine.revision(&AtomicBool::new(false)).is_err());
}

mod build;
