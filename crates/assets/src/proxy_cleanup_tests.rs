use super::*;
struct Root(std::path::PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn setup() -> (Root, ProxyKey) {
    let root = Root(std::env::temp_dir().join(format!("proxy-cleanup-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&root.0).unwrap();
    let project: sungeki_project::Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    let sungeki_project::AssetProbe::Video(probe) = &project.assets[0].probe else {
        panic!()
    };
    let key = ProxyKey::new(probe, b"test").unwrap();
    fs::create_dir(root.0.join(key.as_str())).unwrap();
    (root, key)
}
#[test]
fn completed_corrupt_and_interrupted_entries_are_removable_and_retryable() {
    for files in [
        vec![],
        vec!["video.mov"],
        vec!["video.mov", "manifest.pending"],
        vec!["video.mov", "manifest.json"],
    ] {
        let (root, key) = setup();
        let dir = root.0.join(key.as_str());
        let original = root.0.join("original.mov");
        fs::write(&original, b"original").unwrap();
        for name in files {
            fs::write(dir.join(name), b"unverified cache bytes").unwrap();
        }
        assert!(remove(&root.0, &key, &AtomicBool::new(false)).unwrap());
        assert!(!remove(&root.0, &key, &AtomicBool::new(false)).unwrap());
        assert_eq!(fs::read(original).unwrap(), b"original");
    }
}
#[test]
fn unexpected_entries_reject_before_deleting_known_files() {
    for kind in ["unknown", "nested", "symlink"] {
        let (root, key) = setup();
        let dir = root.0.join(key.as_str());
        fs::write(dir.join("video.mov"), b"keep").unwrap();
        match kind {
            "unknown" => fs::write(dir.join("notes.txt"), b"keep").unwrap(),
            "nested" => fs::create_dir(dir.join("manifest.pending")).unwrap(),
            _ => std::os::unix::fs::symlink("../outside", dir.join("manifest.json")).unwrap(),
        }
        assert!(remove(&root.0, &key, &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read(dir.join("video.mov")).unwrap(), b"keep");
    }
}
#[test]
fn cancellation_and_linked_directories_do_not_delete() {
    let (root, key) = setup();
    let dir = root.0.join(key.as_str());
    fs::write(dir.join("video.mov"), b"keep").unwrap();
    assert_eq!(
        remove(&root.0, &key, &AtomicBool::new(true))
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
    let moved = root.0.join("outside");
    fs::rename(&dir, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &dir).unwrap();
    assert!(remove(&root.0, &key, &AtomicBool::new(false)).is_err());
    assert_eq!(fs::read(moved.join("video.mov")).unwrap(), b"keep");
    let alias = root.0.join("alias");
    std::os::unix::fs::symlink(&root.0, &alias).unwrap();
    assert!(remove(&alias, &key, &AtomicBool::new(false)).is_err());
}
