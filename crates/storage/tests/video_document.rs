use std::fs;
use sungeki_project::{AssetLocator, Project};
use sungeki_storage::{load_document, save_document};
#[test]
fn video_metadata_and_relative_original_binding_survive_document_move() {
    let root = std::env::temp_dir().join(format!("video-document-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    fs::write(
        root.join("original.mov"),
        b"metadata fixture, not a decodable movie",
    )
    .unwrap();
    p.assets[0].locator = AssetLocator::Absolute {
        path: root.join("original.mov").to_str().unwrap().into(),
    };
    save_document(&root.join("project.vvm.json"), &p, None).unwrap();
    assert_eq!(
        load_document(&root.join("project.vvm.json"))
            .unwrap()
            .project,
        p
    );
    let raw: Project =
        serde_json::from_slice(&fs::read(root.join("project.vvm.json")).unwrap()).unwrap();
    assert!(
        matches!(&raw.assets[0].locator,AssetLocator::ProjectRelative {path} if path=="original.mov")
    );
    let moved = root.with_extension("moved");
    fs::rename(&root, &moved).unwrap();
    p.assets[0].locator = AssetLocator::Absolute {
        path: moved.join("original.mov").to_str().unwrap().into(),
    };
    assert_eq!(
        load_document(&moved.join("project.vvm.json"))
            .unwrap()
            .project,
        p
    );
    fs::remove_dir_all(moved).unwrap();
}
