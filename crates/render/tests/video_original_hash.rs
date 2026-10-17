#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
};
use sungeki_project::{AssetLocator, AssetProbe, Project};
use sungeki_render::{RenderError, RenderPlan};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn verify(root: Root, size: u64) {
    fs::create_dir(&root.0).unwrap();
    // Byte-identity fixture only. No claim that these bytes decode as video.
    let path = root.0.join("original.mp4");
    let mut file = fs::File::create(&path).unwrap();
    let chunk = [42u8; 65536];
    let mut hash = Sha256::new();
    let mut remaining = size;
    while remaining > 0 {
        let bytes = &chunk[..remaining.min(chunk.len() as u64) as usize];
        file.write_all(bytes).unwrap();
        hash.update(bytes);
        remaining -= bytes.len() as u64;
    }
    file.sync_all().unwrap();
    let mut project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let asset = &mut project.assets[0];
    asset.locator = AssetLocator::Absolute {
        path: path.to_string_lossy().into(),
    };
    let AssetProbe::Video(probe) = &mut asset.probe else {
        panic!()
    };
    probe.byte_length = size;
    probe.sha256 = format!("{:x}", hash.finalize());
    asset.content_hash = Some(probe.sha256.clone());
    let plan = RenderPlan::new(&project, |_| Ok(())).unwrap();
    assert_eq!(plan.verify_original_hashes().unwrap(), 1);
    file.seek(SeekFrom::Start(size - 1)).unwrap();
    file.write_all(&[43]).unwrap();
    file.sync_all().unwrap();
    assert!(matches!(
        plan.verify_original_hashes(),
        Err(RenderError::SourceChanged(_))
    ));
    file.set_len(size + 1).unwrap();
    assert!(matches!(
        plan.verify_original_hashes(),
        Err(RenderError::SourceChanged(_))
    ));
    drop(file);
    fs::rename(&path, root.0.join("moved")).unwrap();
    assert!(plan.verify_original_hashes().is_err());
    std::os::unix::fs::symlink(root.0.join("moved"), &path).unwrap();
    assert!(plan.verify_original_hashes().is_err());
}
#[test]
fn video_original_identity_rejects_changed_grown_missing_and_symlink_sources() {
    verify(
        Root(std::env::temp_dir().join(format!("video-hash-{}", uuid::Uuid::new_v4()))),
        4096,
    );
}
#[test]
#[ignore = "requires configured external SSD root"]
fn video_larger_than_image_limit_is_verified_without_whole_file_capture() {
    let root = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap());
    verify(
        Root(root.join(format!("large-video-hash-{}", uuid::Uuid::new_v4()))),
        65 * 1024 * 1024,
    );
}
