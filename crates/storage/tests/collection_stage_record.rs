use std::{fs, path::PathBuf};
use sungeki_storage::{
    CollectionStage, CollectionStagePresence as Presence, CollectionStageRecord,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("stage-record-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p.canonicalize().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn child_exits_without_stage_drop() {
    let Some(root) = std::env::var_os("VVM_STAGE_RECORD_CHILD") else {
        return;
    };
    let root = PathBuf::from(root);
    let stage = CollectionStage::new(&root.join("output")).unwrap();
    let record = stage.record().unwrap();
    fs::write(
        root.join("record.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    fs::write(stage.path().join("partial.bin"), b"unfinished").unwrap();
    std::process::exit(73);
}
#[test]
fn ownership_survives_abrupt_exit_and_replacement_is_only_reported() {
    let f = Fixture::new();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_exits_without_stage_drop"])
        .env("VVM_STAGE_RECORD_CHILD", &f.0)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    let record: CollectionStageRecord =
        serde_json::from_slice(&fs::read(f.0.join("record.json")).unwrap()).unwrap();
    assert_eq!(record.inspect().unwrap(), Presence::Owned);
    fs::rename(record.path(), f.0.join("original-stage")).unwrap();
    fs::create_dir(record.path()).unwrap();
    fs::write(record.path().join("keep"), b"user data").unwrap();
    assert_eq!(record.inspect().unwrap(), Presence::Changed);
    assert_eq!(fs::read(record.path().join("keep")).unwrap(), b"user data");
    fs::remove_file(record.path().join("keep")).unwrap();
    fs::remove_dir(record.path()).unwrap();
    assert_eq!(record.inspect().unwrap(), Presence::Missing);
    std::os::unix::fs::symlink(f.0.join("original-stage"), record.path()).unwrap();
    assert_eq!(record.inspect().unwrap(), Presence::Changed);
    assert_eq!(
        fs::read(f.0.join("original-stage/partial.bin")).unwrap(),
        b"unfinished"
    );
}
#[test]
fn publication_drop_and_unsafe_serialized_path_are_distinguished() {
    let f = Fixture::new();
    let stage = CollectionStage::new(&f.0.join("output")).unwrap();
    let record = stage.record().unwrap();
    fs::create_dir(stage.path().join("assets")).unwrap();
    for p in ["project.vvm.json", "manifest.json"] {
        fs::write(stage.path().join(p), b"{}").unwrap();
    }
    stage.commit(|| false).unwrap();
    assert_eq!(record.inspect().unwrap(), Presence::Missing);
    assert!(record.destination().is_dir());
    let stage = CollectionStage::new(&f.0.join("other")).unwrap();
    let record = stage.record().unwrap();
    drop(stage);
    assert_eq!(record.inspect().unwrap(), Presence::Missing);
    let mut value = serde_json::to_value(&record).unwrap();
    value["path"] = "/tmp/user-data".into();
    let invalid: CollectionStageRecord = serde_json::from_value(value).unwrap();
    assert!(invalid.inspect().is_err());
}
