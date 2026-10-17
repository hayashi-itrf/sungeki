use std::fs;
use sungeki_storage::{PublishError, read_collection_metadata};
#[test]
fn bounded_metadata_read_rejects_symlink_growth_and_cancellation() {
    let root = std::env::temp_dir().join(format!("collection-read-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("metadata.json");
    let bytes = vec![42; 200_000];
    fs::write(&path, &bytes).unwrap();
    assert_eq!(read_collection_metadata(&path, || false).unwrap(), bytes);
    assert!(matches!(
        read_collection_metadata(&path, || true),
        Err(PublishError::Cancelled)
    ));
    let link = root.join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(read_collection_metadata(&link, || false).is_err());
    let mut calls = 0;
    assert!(
        read_collection_metadata(&path, || {
            calls += 1;
            if calls == 3 {
                fs::write(&path, vec![7; 200_001]).unwrap();
            }
            false
        })
        .is_err()
    );
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(read_collection_metadata(&path, || false).is_err());
    fs::remove_dir_all(root).unwrap();
}
