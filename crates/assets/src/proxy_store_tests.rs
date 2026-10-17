use super::*;
use std::os::unix::fs::symlink;
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn setup() -> (Root, PathBuf, ProxyManifest) {
    let root = Root(std::env::temp_dir().join(format!("proxy-store-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&root.0).unwrap();
    let project: sungeki_project::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    let sungeki_project::AssetProbe::Video(mut source) = project.assets[0].probe.clone() else {
        panic!()
    };
    source.metadata.video.frame_count = Some(2);
    let file = root.0.join("generated.mov");
    let bytes = b"identity-only fixture; not a decoded video";
    fs::write(&file, bytes).unwrap();
    let mut derived = source.clone();
    derived.sha256 = format!("{:x}", Sha256::digest(bytes));
    derived.byte_length = bytes.len() as u64;
    (
        root,
        file,
        ProxyManifest {
            version: crate::proxy::PROXY_FORMAT_VERSION,
            source,
            derived,
            conversion: "test-descriptor".into(),
            encoder_sha256: "b".repeat(64),
            source_pts: vec![0, 3000],
            proxy_pts: vec![0, 3000],
        },
    )
}

#[test]
fn persistence_roundtrip_no_clobber_and_tamper_rejection() {
    let (root, original, manifest) = setup();
    let cancel = AtomicBool::new(false);
    let key = manifest.key().unwrap();
    let dir = publish(&root.0, &manifest, &original, &cancel).unwrap();
    assert_eq!(read_candidate(&root.0, &key, &cancel).unwrap(), manifest);
    assert_eq!(
        publish(&root.0, &manifest, &original, &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    let bytes = fs::read(&original).unwrap();
    fs::write(dir.join("video.mov"), vec![b'x'; bytes.len()]).unwrap();
    assert!(read_candidate(&root.0, &key, &cancel).is_err());
    fs::remove_file(dir.join("video.mov")).unwrap();
    symlink(&original, dir.join("video.mov")).unwrap();
    assert!(read_candidate(&root.0, &key, &cancel).is_err());
    assert_eq!(fs::read(&original).unwrap(), bytes);
    fs::remove_file(dir.join("video.mov")).unwrap();
    fs::write(dir.join("video.mov"), bytes).unwrap();
    let mut wrong = manifest.clone();
    wrong.encoder_sha256 = "c".repeat(64);
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&wrong).unwrap(),
    )
    .unwrap();
    assert!(read_candidate(&root.0, &key, &cancel).is_err());
}

#[test]
fn incomplete_entries_ignored_failures_cleaned_original_untouched() {
    let (root, original, manifest) = setup();
    let key = manifest.key().unwrap();
    let cancel = AtomicBool::new(true);
    assert_eq!(
        publish(&root.0, &manifest, &original, &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    assert!(!root.0.join(key.as_str()).exists());
    cancel.store(false, Ordering::Release);
    let before = fs::read(&original).unwrap();
    let mut wrong = manifest.clone();
    wrong.derived.sha256 = "0".repeat(64);
    assert!(publish(&root.0, &wrong, &original, &cancel).is_err());
    assert!(!root.0.join(key.as_str()).exists());
    let partial = root.0.join(key.as_str());
    fs::create_dir(&partial).unwrap();
    fs::write(partial.join("manifest.pending"), b"partial").unwrap();
    assert!(read_candidate(&root.0, &key, &cancel).is_err());
    assert_eq!(
        publish(&root.0, &manifest, &original, &cancel)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(
        fs::read(partial.join("manifest.pending")).unwrap(),
        b"partial"
    );
    assert_eq!(fs::read(&original).unwrap(), before);
}

#[test]
fn descriptor_version_and_frame_bounds_are_validated() {
    let (_root, _original, manifest) = setup();
    for edit in [0, 1, 2, 3] {
        let mut m = manifest.clone();
        match edit {
            0 => m.version += 1,
            1 => m.source_pts.clear(),
            2 => m.proxy_pts = vec![0, 0],
            _ => m.encoder_sha256 = "../bad".into(),
        }
        assert!(m.key().is_err());
    }
    let mut changed = manifest.clone();
    changed.encoder_sha256 = "c".repeat(64);
    assert_ne!(changed.key().unwrap(), manifest.key().unwrap());
}

#[test]
fn manifest_accepts_long_video_budget_but_rejects_over_one_gib() {
    let (_root, _original, mut manifest) = setup();
    manifest.derived.byte_length = 512 * 1024 * 1024;
    assert!(manifest.key().is_ok());
    manifest.derived.byte_length = 1024 * 1024 * 1024;
    assert!(manifest.key().is_ok());
    manifest.derived.byte_length += 1;
    assert!(manifest.key().is_err());
}
