use super::*;
use crate::speech_journal_tests::Fixture;
use std::{fs, os::unix::fs::symlink};
pub(super) fn paths(root: &std::path::Path) -> Paths {
    let paths = Paths {
        model: root.join("model"),
        config: root.join("config.json"),
        style_vectors: root.join("styles.npy"),
        text_dependencies: BTreeMap::from([
            ("tokenizer".into(), root.join("tokenizer")),
            ("encoder".into(), root.join("encoder")),
        ]),
    };
    fs::write(&paths.model, b"model-original").unwrap();
    fs::write(&paths.style_vectors, b"style-original").unwrap();
    fs::write(
        &paths.config,
        serde_json::to_vec(&serde_json::json!({
            "version":"2.7.0-JP-Extra", "data":{"sampling_rate":44100,"spk2id":{"voice":0},
            "num_styles":1,"style2id":{"Neutral":0}}
        }))
        .unwrap(),
    )
    .unwrap();
    for (name, path) in &paths.text_dependencies {
        fs::write(path, name).unwrap();
    }
    paths
}
#[test]
fn measures_files_and_detects_same_length_replacement_and_dependency_changes() {
    let fixture = Fixture::new();
    let paths = paths(&fixture.0);
    let cancel = AtomicBool::new(false);
    let measured = inspect(&paths, &cancel).unwrap();
    let expected = Sbv2Identity {
        engine_revision: "separately-attested".into(),
        model_sha256: measured.model_sha256.clone(),
        config_sha256: measured.config_sha256.clone(),
        style_vectors_sha256: measured.style_vectors_sha256.clone(),
        text_encoder_fingerprint: measured.text_encoder_fingerprint.clone(),
    };
    measured.verify_files(&expected).unwrap();
    assert_eq!(
        measured.model_sha256,
        format!("{:x}", Sha256::digest(b"model-original"))
    );
    assert_eq!(
        ModelConfig::parse(measured.config_text.as_bytes())
            .unwrap()
            .sha256(),
        measured.config_sha256
    );
    fs::write(&paths.model, b"model-modified").unwrap();
    assert!(
        inspect(&paths, &cancel)
            .unwrap()
            .verify_files(&expected)
            .is_err()
    );
    fs::write(&paths.model, b"model-original").unwrap();
    fs::write(&paths.text_dependencies["tokenizer"], b"modified").unwrap();
    assert!(
        inspect(&paths, &cancel)
            .unwrap()
            .verify_files(&expected)
            .is_err()
    );
    fs::write(&paths.text_dependencies["tokenizer"], b"tokenizer").unwrap();
    fs::write(&paths.style_vectors, b"style-modified").unwrap();
    assert!(
        inspect(&paths, &cancel)
            .unwrap()
            .verify_files(&expected)
            .is_err()
    );
}
#[test]
fn missing_link_directory_fifo_oversized_empty_and_cancelled_reads_are_rejected() {
    let fixture = Fixture::new();
    let regular = fixture.0.join("regular");
    fs::write(&regular, b"123456789").unwrap();
    let link = fixture.0.join("link");
    symlink(&regular, &link).unwrap();
    let fifo = fixture.0.join("fifo");
    let encoded = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: this temporary path is NUL terminated and owned by the fixture.
    assert_eq!(unsafe { libc::mkfifo(encoded.as_ptr(), 0o600) }, 0);
    let empty = fixture.0.join("empty");
    fs::write(&empty, b"").unwrap();
    for path in [&link, &fifo, &empty, &fixture.0, &fixture.0.join("missing")] {
        assert!(file::inspect(path, 16, false, &AtomicBool::new(false)).is_err());
    }
    assert!(file::inspect(&regular, 8, false, &AtomicBool::new(false)).is_err());
    assert_eq!(
        file::inspect(&regular, 16, false, &AtomicBool::new(true))
            .err()
            .unwrap()
            .code,
        ErrorCode::Cancelled
    );
    let streamed = file::inspect(&regular, 16, false, &AtomicBool::new(false)).unwrap();
    assert!(streamed.bytes.is_empty());
    assert_eq!(streamed.len, 9);
    assert_eq!(
        file::inspect(&regular, 16, true, &AtomicBool::new(false))
            .unwrap()
            .bytes,
        b"123456789"
    );
}
#[test]
fn manifest_uses_logical_names_and_config_is_parsed_before_acceptance() {
    let fixture = Fixture::new();
    let mut paths = paths(&fixture.0);
    let cancel = AtomicBool::new(false);
    let before = inspect(&paths, &cancel).unwrap();
    let renamed = fixture.0.join("relocated-tokenizer");
    fs::rename(&paths.text_dependencies["tokenizer"], &renamed).unwrap();
    paths.text_dependencies.insert("tokenizer".into(), renamed);
    assert_eq!(
        inspect(&paths, &cancel).unwrap().text_encoder_fingerprint,
        before.text_encoder_fingerprint
    );
    let file = paths.text_dependencies.remove("tokenizer").unwrap();
    paths
        .text_dependencies
        .insert("different-role".into(), file);
    assert_ne!(
        inspect(&paths, &cancel).unwrap().text_encoder_fingerprint,
        before.text_encoder_fingerprint
    );
    fs::write(&paths.config, b"{}").unwrap();
    assert!(inspect(&paths, &cancel).is_err());
    paths.text_dependencies.clear();
    assert!(inspect(&paths, &cancel).is_err());
}
