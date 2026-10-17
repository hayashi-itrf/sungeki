use crate::originals::{cancelled, read_verified, write_new};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::symlink, sync::atomic::AtomicBool};

#[test]
fn captured_bytes_reject_changed_size_hash_symlink_and_existing_output() {
    let directory = std::env::temp_dir().join(format!("vvm-render-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let source = directory.join("source");
    let hash = format!("{:x}", Sha256::digest(b"original"));
    write_new(&source, b"original").unwrap();
    assert_eq!(read_verified(&source, &hash, Some(8)).unwrap(), b"original");
    assert!(read_verified(&source, &hash, Some(7)).is_err());
    assert!(write_new(&source, b"overwrite").is_err());
    fs::write(&source, b"modified").unwrap();
    assert!(read_verified(&source, &hash, Some(8)).is_err());
    let link = directory.join("link");
    symlink(&source, &link).unwrap();
    let modified = format!("{:x}", Sha256::digest(b"modified"));
    assert!(read_verified(&link, &modified, None).is_err());
    assert!(cancelled(&AtomicBool::new(true)).is_err());
    fs::remove_dir_all(&directory).unwrap();
}
