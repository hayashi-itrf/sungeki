use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::symlink};
use sungeki_storage::{PublishError, publish_png_output, read_verified_output};

#[test]
fn frame_ranges_validate_the_whole_file_and_png_publication_never_overwrites() {
    let root = std::env::temp_dir().join(format!("vvm-frame-access-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("frame.png");
    let target = root.join("保存画像.png");
    let bytes: Vec<_> = (0..400_000).map(|i| (i % 251) as u8).collect();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    fs::write(&source, &bytes).unwrap();
    let read = |offset, length| {
        read_verified_output(&source, &hash, bytes.len() as u64, offset, length, || false)
    };
    assert_eq!(read(131_000, 512).unwrap(), bytes[131_000..131_512]);
    assert_eq!(read(399_995, 512).unwrap(), bytes[399_995..]);
    for (offset, length) in [(400_000, 1), (u64::MAX, 1), (0, 0), (0, 262_145)] {
        assert!(matches!(
            read(offset, length),
            Err(PublishError::InvalidRange)
        ));
    }
    assert!(matches!(
        read_verified_output(&source, &hash, bytes.len() as u64, 0, 1, || true),
        Err(PublishError::Cancelled)
    ));
    let publish = |path: &std::path::Path| {
        publish_png_output(&source, path, &hash, bytes.len() as u64, || false)
    };
    assert!(matches!(
        publish(&root.join("bad.mp4")),
        Err(PublishError::InvalidDestination)
    ));
    assert!(!publish(&target).unwrap().already_present);
    assert!(publish(&target).unwrap().already_present);
    fs::write(&target, b"keep").unwrap();
    assert!(matches!(publish(&target), Err(PublishError::OutputExists)));
    assert_eq!(fs::read(&target).unwrap(), b"keep");
    let mut changed = bytes.clone();
    changed[399_999] ^= 1;
    fs::write(&source, &changed).unwrap();
    // Tampering outside the requested first byte must also be rejected.
    assert!(matches!(read(0, 1), Err(PublishError::Changed)));
    assert!(matches!(
        publish(&root.join("changed.png")),
        Err(PublishError::Changed)
    ));
    fs::remove_file(&source).unwrap();
    symlink(&target, &source).unwrap();
    assert!(matches!(read(0, 1), Err(PublishError::Changed)));
    fs::remove_file(&source).unwrap();
    fs::write(&source, &bytes).unwrap();
    let mut checks = 0;
    assert!(matches!(
        read_verified_output(&source, &hash, bytes.len() as u64, 0, 1, || {
            checks += 1;
            if checks == 3 {
                fs::write(&source, &changed).unwrap();
            }
            false
        }),
        Err(PublishError::Changed)
    ));
    fs::remove_dir_all(root).unwrap();
}
