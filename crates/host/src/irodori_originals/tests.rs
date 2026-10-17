use super::*;
use std::{fs, os::unix::fs::symlink};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("irodori-originals-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn paths(f: &Fixture) -> Paths {
    for name in ["model", "codec", "engine", "voice"] {
        fs::write(f.0.join(name), name).unwrap();
    }
    Paths {
        model: BTreeMap::from([("model".into(), f.0.join("model"))]),
        codec: BTreeMap::from([("codec".into(), f.0.join("codec"))]),
        engine: BTreeMap::from([("engine".into(), f.0.join("engine"))]),
        voice: BTreeMap::from([("voice".into(), f.0.join("voice"))]),
    }
}
fn identity(m: &Measured) -> RuntimeIdentity {
    RuntimeIdentity {
        checkpoint: "irodori".into(),
        codec_manifest_sha256: m.codec_manifest_sha256.clone(),
        model_manifest_sha256: m.model_manifest_sha256.clone(),
        engine_manifest_sha256: m.engine_manifest_sha256.clone(),
        voice_manifest_sha256: m.voice_manifest_sha256.clone(),
        sample_rate: 24000,
        channels: 1,
    }
}
#[test]
fn detects_same_length_changes_in_each_original_group() {
    let f = Fixture::new();
    let paths = paths(&f);
    let cancel = AtomicBool::new(false);
    let baseline = identity(&inspect(&paths, &cancel).unwrap());
    for name in ["model", "codec", "engine", "voice"] {
        fs::write(f.0.join(name), "X".repeat(name.len())).unwrap();
        assert!(
            inspect(&paths, &cancel)
                .unwrap()
                .verify_files(&baseline)
                .is_err()
        );
        fs::write(f.0.join(name), name).unwrap();
        inspect(&paths, &cancel)
            .unwrap()
            .verify_files(&baseline)
            .unwrap();
    }
}
#[test]
fn physical_relocation_keeps_identity_but_logical_names_and_membership_do_not() {
    let f = Fixture::new();
    let mut paths = paths(&f);
    let cancel = AtomicBool::new(false);
    let baseline = identity(&inspect(&paths, &cancel).unwrap());
    fs::rename(f.0.join("model"), f.0.join("moved")).unwrap();
    paths.model.insert("model".into(), f.0.join("moved"));
    inspect(&paths, &cancel)
        .unwrap()
        .verify_files(&baseline)
        .unwrap();
    let path = paths.model.remove("model").unwrap();
    paths.model.insert("renamed".into(), path);
    assert!(
        inspect(&paths, &cancel)
            .unwrap()
            .verify_files(&baseline)
            .is_err()
    );
    fs::write(f.0.join("empty.py"), b"").unwrap();
    paths.engine.insert("empty.py".into(), f.0.join("empty.py"));
    assert!(
        inspect(&paths, &cancel)
            .unwrap()
            .verify_files(&baseline)
            .is_err()
    );
}
#[test]
fn rejects_symlink_directory_empty_manifest_and_cancellation() {
    let f = Fixture::new();
    let mut paths = paths(&f);
    let cancel = AtomicBool::new(false);
    assert!(inspect(&paths, &AtomicBool::new(true)).is_err());
    symlink(f.0.join("model"), f.0.join("link")).unwrap();
    paths.model.insert("model".into(), f.0.join("link"));
    assert!(inspect(&paths, &cancel).is_err());
    paths.model.insert("model".into(), f.0.clone());
    assert!(inspect(&paths, &cancel).is_err());
    paths.model.clear();
    assert!(inspect(&paths, &cancel).is_err());
}

#[test]
fn codec_is_required_and_role_hashes_cannot_be_substituted() {
    let f = Fixture::new();
    let mut paths = paths(&f);
    let cancel = AtomicBool::new(false);
    // Equal contents and logical names still have distinct role identities.
    paths.codec = paths.model.clone();
    let measured = inspect(&paths, &cancel).unwrap();
    assert_ne!(
        measured.codec_manifest_sha256,
        measured.model_manifest_sha256
    );
    let mut expected = identity(&measured);
    expected.codec_manifest_sha256 = measured.model_manifest_sha256.clone();
    assert_eq!(
        measured.verify_files(&expected).unwrap_err().code,
        ErrorCode::ExternalModification
    );
    paths.codec.clear();
    assert!(inspect(&paths, &cancel).is_err());
}

#[test]
fn empty_source_is_valid_but_empty_weights_and_invalid_labels_are_not() {
    let f = Fixture::new();
    let mut paths = paths(&f);
    let cancel = AtomicBool::new(false);
    fs::write(f.0.join("engine"), b"").unwrap();
    inspect(&paths, &cancel).unwrap();
    fs::write(f.0.join("codec"), b"").unwrap();
    assert!(inspect(&paths, &cancel).is_err());
    fs::write(f.0.join("codec"), b"codec").unwrap();
    paths
        .codec
        .insert("invalid\nlabel".into(), f.0.join("codec"));
    assert!(inspect(&paths, &cancel).is_err());
}
