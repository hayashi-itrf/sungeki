use super::*;
use crate::speech_journal_tests::Fixture;
use std::{fs, os::unix::fs::symlink, path::PathBuf};
fn setup() -> (Fixture, PathBuf, PathBuf, PathBuf) {
    let fixture = Fixture::new();
    let root = fs::canonicalize(&fixture.0).unwrap();
    fs::create_dir_all(root.join("venv/bin")).unwrap();
    let invocation = root.join("venv/bin/python");
    let target = root.join("interpreter");
    let config = root.join("venv/pyvenv.cfg");
    fs::write(&target, b"test executable, not executed").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&config, b"include-system-site-packages = false\n").unwrap();
    symlink(&target, &invocation).unwrap();
    (fixture, invocation, target, config)
}
#[test]
fn venv_path_resolves_to_declared_binary_and_config_participates_in_identity() {
    let (_fixture, invocation, target, config) = setup();
    let original = inspect(&invocation, &target, &AtomicBool::new(false)).unwrap();
    fs::write(
        &config,
        b"include-system-site-packages = false\nhome = /another/base\n",
    )
    .unwrap();
    let changed = inspect(&invocation, &target, &AtomicBool::new(false)).unwrap();
    assert_eq!(original.0, changed.0);
    assert_ne!(original.1, changed.1);
    // The invocation path remains the venv path, not the canonical target.
    assert_ne!(invocation, fs::canonicalize(&invocation).unwrap());
}
#[test]
fn changed_link_is_rejected_even_when_the_other_binary_has_identical_bytes() {
    let (fixture, invocation, target, _) = setup();
    let other = fs::canonicalize(&fixture.0)
        .unwrap()
        .join("another-interpreter");
    fs::copy(&target, &other).unwrap();
    fs::remove_file(&invocation).unwrap();
    symlink(&other, &invocation).unwrap();
    assert!(inspect(&invocation, &target, &AtomicBool::new(false)).is_err());
}
#[test]
fn missing_shared_ambiguous_or_linked_venv_config_is_rejected() {
    let (fixture, invocation, target, config) = setup();
    for text in [
        "home = /base\n",
        "include-system-site-packages = true\n",
        "include-system-site-packages = false\ninclude-system-site-packages = true\n",
    ] {
        fs::write(&config, text).unwrap();
        assert!(inspect(&invocation, &target, &AtomicBool::new(false)).is_err());
    }
    fs::remove_file(&config).unwrap();
    assert!(inspect(&invocation, &target, &AtomicBool::new(false)).is_err());
    let other = fixture.0.join("config-copy");
    fs::write(&other, b"include-system-site-packages = false\n").unwrap();
    symlink(&other, &config).unwrap();
    assert!(inspect(&invocation, &target, &AtomicBool::new(false)).is_err());
}
