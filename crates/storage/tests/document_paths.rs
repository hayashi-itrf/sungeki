use std::{fs, path::PathBuf};
use sungeki_project::{AssetLocator, Project};
use sungeki_storage::*;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("portable-doc-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn project(&self, name: &str) -> Project {
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        p.assets[0].locator = AssetLocator::Absolute {
            path: self.0.join(name).to_str().unwrap().into(),
        };
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn moves_document_and_its_assets_then_saves_again_without_old_absolute_paths() {
    let root = Fixture::new();
    let bundle = root.0.join("bundle");
    fs::create_dir(&bundle).unwrap();
    fs::create_dir(bundle.join("assets")).unwrap();
    fs::write(bundle.join("assets/source.psd"), "identity fixture only").unwrap();
    let project = root.project("bundle/assets/source.psd");
    let before = project.clone();
    let path = bundle.join("project.vvm.json");
    save_document(&path, &project, None).unwrap();
    let raw: Project = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        raw.assets[0].locator,
        AssetLocator::ProjectRelative {
            path: "assets/source.psd".into()
        }
    );
    assert_eq!(load_document(&path).unwrap().project, project);
    assert_eq!(project, before);
    let moved = root.0.join("moved");
    fs::rename(&bundle, &moved).unwrap();
    assert!(!bundle.exists());
    let path = moved.join("project.vvm.json");
    let loaded = load_document(&path).unwrap();
    assert_eq!(
        loaded.project.assets[0].locator,
        AssetLocator::Absolute {
            path: moved.join("assets/source.psd").to_str().unwrap().into()
        }
    );
    save_document(&path, &loaded.project, Some(&loaded.stamp)).unwrap();
    let raw = fs::read_to_string(&path).unwrap();
    assert!(!raw.contains(root.0.to_str().unwrap()));
    assert_eq!(load_document(&path).unwrap().project, loaded.project);
}
#[test]
fn save_as_outside_the_bundle_preserves_the_exact_existing_source_reference() {
    let root = Fixture::new();
    fs::create_dir(root.0.join("other")).unwrap();
    let project = root.project("missing.psd");
    let path = root.0.join("other/project.vvm.json");
    save_document(&path, &project, None).unwrap();
    assert_eq!(load_document(&path).unwrap().project, project);
    let raw: Project = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(raw, project);
}
#[test]
fn rejects_escape_ambiguous_paths_and_symlink_components_but_allows_missing_assets() {
    let root = Fixture::new();
    let path = root.0.join("project.vvm.json");
    for value in [
        "../secret.psd",
        "/tmp/secret.psd",
        "assets/../secret.psd",
        "assets\\secret.psd",
        "C:secret.psd",
        "assets//secret.psd",
        "assets/./secret.psd",
    ] {
        let mut p = root.project("placeholder");
        p.assets[0].locator = AssetLocator::ProjectRelative { path: value.into() };
        fs::write(&path, serde_json::to_vec(&p).unwrap()).unwrap();
        assert!(load_document(&path).is_err(), "{value}");
    }
    let mut p = root.project("placeholder");
    p.assets[0].locator = AssetLocator::ProjectRelative {
        path: "assets/missing.psd".into(),
    };
    fs::write(&path, serde_json::to_vec(&p).unwrap()).unwrap();
    assert!(load_document(&path).is_ok());
    std::os::unix::fs::symlink("/tmp", root.0.join("assets")).unwrap();
    assert!(load_document(&path).is_err());
}

#[test]
fn unusual_absolute_references_are_preserved_without_changing_snapshot_hash() {
    let root = Fixture::new();
    for (i, source) in [
        "assets/../missing.psd",
        "./missing.psd",
        "assets//missing.psd",
    ]
    .iter()
    .enumerate()
    {
        let project = root.project(source);
        let path = root.0.join(format!("unusual-{i}.vvm.json"));
        save_document(&path, &project, None).unwrap();
        assert_eq!(load_document(&path).unwrap().project, project);
    }
    std::os::unix::fs::symlink("/tmp", root.0.join("linked")).unwrap();
    let project = root.project("linked/missing.psd");
    let path = root.0.join("symlink.vvm.json");
    save_document(&path, &project, None).unwrap();
    assert_eq!(load_document(&path).unwrap().project, project);
}
