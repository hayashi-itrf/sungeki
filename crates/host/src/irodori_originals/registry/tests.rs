use super::*;
use serde_json::json;
use std::{fs, os::unix::fs::symlink};
#[test]
fn every_reference_is_bound_and_registry_changes_are_observable() {
    let root = std::env::temp_dir().join(format!("irodori-reg-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    struct Clean(PathBuf);
    impl Drop for Clean {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _clean = Clean(root.clone());
    let registry = root.join("voices.json");
    let audio = root.join("reference.wav");
    fs::write(&audio, "fixture").unwrap();
    let value = json!([
        {"id":"audio","name":"音声","reference":"audio","caption":null,"path":audio},
        {"id":"latent","name":"潜在","reference":"latent","caption":"声の説明","path":audio},
        {"id":"none","name":"参照なし","reference":"none","caption":null}
    ]);
    fs::write(&registry, serde_json::to_vec(&value).unwrap()).unwrap();
    let files = BTreeMap::from([
        ("registry".into(), registry.clone()),
        ("reference:audio".into(), audio.clone()),
        ("reference:latent".into(), audio.clone()),
    ]);
    let flag = AtomicBool::new(false);
    let first = read(&registry, &files, &flag).unwrap();
    assert_eq!(first.voices.len(), 3);
    for mode in 0..7 {
        let mut changed = value.clone();
        match mode {
            0 => changed[0]["path"] = json!("relative.wav"),
            1 => changed[2]["path"] = json!(audio),
            2 => {
                changed[0].as_object_mut().unwrap().remove("path");
            }
            3 => changed[1]["id"] = "audio".into(),
            4 => {
                changed[0].as_object_mut().unwrap().remove("caption");
            }
            5 => changed[0]["legacy_voice"] = true.into(),
            _ => changed[0]["caption"] = " ".into(),
        }
        fs::write(&registry, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(read(&registry, &files, &flag).is_err(), "{mode}");
    }
    fs::write(&registry, serde_json::to_vec(&value).unwrap()).unwrap();
    let mut missing = files.clone();
    missing.remove("reference:latent");
    assert!(read(&registry, &missing, &flag).is_err());
    missing.insert("extra".into(), audio.clone());
    assert!(read(&registry, &missing, &flag).is_err());
    assert!(read(&registry, &files, &AtomicBool::new(true)).is_err());
    let mut changed = value.clone();
    changed[0]["name"] = "別名".into();
    fs::write(&registry, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert_ne!(read(&registry, &files, &flag).unwrap().sha256, first.sha256);
    symlink(&audio, root.join("alias")).unwrap();
    changed[0]["path"] = json!(root.join("alias"));
    fs::write(&registry, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(read(&registry, &files, &flag).is_err());
}
