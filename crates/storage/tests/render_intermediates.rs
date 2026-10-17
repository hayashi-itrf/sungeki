use std::{fs, path::PathBuf};
use sungeki_storage::RenderIntermediates;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("render-cleanup-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn removes_only_known_intermediates_preserves_outputs_records_and_originals() {
    let f = Fixture::new();
    let work = f.0.join("work");
    fs::create_dir(&work).unwrap();
    let cleanup = RenderIntermediates::new(&work).unwrap();
    for name in [
        "frame-0.png",
        "source-2",
        "mix.wav",
        "segment-12.mp4",
        "font.ttc",
        "decoded-source-1.wav",
    ] {
        fs::write(work.join(name), b"temp").unwrap();
    }
    for name in [
        "output.mp4",
        "verification.json",
        "report-0.json",
        "frame-0.json",
        "unknown.png",
        "source-user",
    ] {
        fs::write(work.join(name), b"keep").unwrap();
    }
    fs::write(f.0.join("original.wav"), b"original").unwrap();
    assert_eq!(cleanup.capture().unwrap().remove().unwrap(), 24);
    assert_eq!(fs::read_dir(&work).unwrap().count(), 6);
    assert_eq!(fs::read(f.0.join("original.wav")).unwrap(), b"original");
}
#[test]
fn dropping_uncommitted_cleanup_does_not_delete() {
    let f = Fixture::new();
    fs::write(f.0.join("mix.wav"), b"keep").unwrap();
    drop(RenderIntermediates::new(&f.0).unwrap().capture().unwrap());
    assert!(f.0.join("mix.wav").exists());
}
#[test]
fn replaced_directory_and_symlink_never_redirect_removal() {
    let f = Fixture::new();
    let work = f.0.join("work");
    fs::create_dir(&work).unwrap();
    fs::write(work.join("mix.wav"), b"temp").unwrap();
    let cleanup = RenderIntermediates::new(&work).unwrap().capture().unwrap();
    fs::rename(&work, f.0.join("owned")).unwrap();
    fs::create_dir(&work).unwrap();
    fs::write(work.join("mix.wav"), b"unrelated").unwrap();
    assert_eq!(cleanup.remove().unwrap(), 4);
    assert_eq!(fs::read(work.join("mix.wav")).unwrap(), b"unrelated");
    let cleanup = RenderIntermediates::new(&work).unwrap().capture().unwrap();
    fs::remove_file(work.join("mix.wav")).unwrap();
    fs::write(f.0.join("original"), b"protected").unwrap();
    std::os::unix::fs::symlink(f.0.join("original"), work.join("mix.wav")).unwrap();
    assert!(cleanup.remove().is_err());
    assert_eq!(fs::read(f.0.join("original")).unwrap(), b"protected");
}
#[test]
fn capture_rejects_known_symlinks_and_hardlinks() {
    let f = Fixture::new();
    fs::write(f.0.join("original"), b"keep").unwrap();
    fs::hard_link(f.0.join("original"), f.0.join("mix.wav")).unwrap();
    assert!(RenderIntermediates::new(&f.0).unwrap().capture().is_err());
    fs::remove_file(f.0.join("mix.wav")).unwrap();
    std::os::unix::fs::symlink(f.0.join("original"), f.0.join("mix.wav")).unwrap();
    assert!(RenderIntermediates::new(&f.0).unwrap().capture().is_err());
}
#[test]
fn changed_file_is_retained_even_when_size_is_unchanged() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let path = f.0.join("mix.wav");
    fs::write(&path, b"temp").unwrap();
    let cleanup = RenderIntermediates::new(&f.0).unwrap().capture().unwrap();
    fs::write(&path, b"edit").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(cleanup.remove().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"edit");
}
