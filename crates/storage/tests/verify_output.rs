use sha2::{Digest, Sha256};
use std::fs;
use sungeki_storage::PublishError;

#[test]
fn verify_without_copy_is_bounded_cancellable_and_observes_mid_read_change() {
    let root = std::env::temp_dir().join(format!("vvm-verify-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let source = root.join("source.mp4");
    let bytes: Vec<_> = (0..400_000).map(|i| (i % 251) as u8).collect();
    fs::write(&source, &bytes).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let verify =
        |cancel| sungeki_storage::verify_output(&source, &hash, bytes.len() as u64, cancel);
    verify(Box::new(|| false) as Box<dyn FnMut() -> bool>).unwrap();
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    let mut calls = 0;
    assert!(matches!(
        verify(Box::new(|| {
            calls += 1;
            calls == 3
        })),
        Err(PublishError::Cancelled)
    ));
    let mut calls = 0;
    assert!(matches!(
        verify(Box::new(|| {
            calls += 1;
            if calls == 3 {
                fs::write(&source, vec![0; bytes.len()]).unwrap();
            }
            false
        })),
        Err(PublishError::Changed)
    ));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}
