#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{cell::Cell, fs};
use sungeki_media::{WaveformCacheError, source_identity};
fn fixture() -> (std::path::PathBuf, std::path::PathBuf, Vec<u8>) {
    let root = std::env::temp_dir().join(format!("audio-identity-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("source");
    let bytes = vec![19; 200000];
    fs::write(&path, &bytes).unwrap();
    (root, path, bytes)
}
#[test]
fn streams_original_identity_without_claiming_audio_decoding() {
    let (root, path, bytes) = fixture();
    let result = source_identity(&path, 200000, || false).unwrap();
    assert_eq!(
        result,
        (format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64)
    );
    assert!(matches!(
        source_identity(&path, 199999, || false),
        Err(WaveformCacheError::Wav(sungeki_media::WavError::TooLarge))
    ));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn cancellation_is_observed_between_chunks() {
    let (root, path, _) = fixture();
    let calls = Cell::new(0);
    let result = source_identity(&path, 200000, || {
        calls.set(calls.get() + 1);
        calls.get() >= 3
    });
    assert!(matches!(result, Err(WaveformCacheError::Cancelled)));
    assert_eq!(calls.get(), 3);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn same_size_mutation_and_replacement_during_scan_fail_closed() {
    for replacement in [false, true] {
        let (root, path, bytes) = fixture();
        let calls = Cell::new(0);
        let result = source_identity(&path, 200000, || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                if replacement {
                    fs::rename(&path, root.join("previous")).unwrap();
                }
                fs::write(&path, vec![20; bytes.len()]).unwrap();
            }
            false
        });
        assert!(
            matches!(result, Err(WaveformCacheError::SourceChanged)),
            "{result:?}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn symlinks_and_nonregular_files_are_not_sources() {
    let (root, path, _) = fixture();
    let link = root.join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(source_identity(&link, 200000, || false).is_err());
    assert!(source_identity(&root, 200000, || false).is_err());
    fs::remove_dir_all(root).unwrap();
}
