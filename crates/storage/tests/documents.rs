use std::{fs, path::PathBuf};
use sungeki_project::Project;
use sungeki_storage::*;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("vvm-document-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn project() -> Project {
    Project::empty(uuid::Uuid::new_v4().to_string(), "保存試験".into())
}
#[test]
fn save_reload_backup_and_external_change() {
    let dir = Directory::new();
    let path = dir.0.join("project.vvm.json");
    let mut p = project();
    let initial = save_document(&path, &p, None).unwrap();
    assert_eq!(load_document(&path).unwrap().project, p);
    assert!(matches!(
        save_document(&path, &p, None),
        Err(DocumentError::OutputExists)
    ));
    p.title = "変更後".into();
    let second = save_document(&path, &p, Some(&initial)).unwrap();
    let backup: Project =
        serde_json::from_slice(&fs::read(dir.0.join("project.vvm.json.bak")).unwrap()).unwrap();
    assert_eq!(backup.title, "保存試験");
    fs::write(&path, b"external edit").unwrap();
    assert!(matches!(
        save_document(&path, &p, Some(&second)),
        Err(DocumentError::ExternalModification)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"external edit");
    assert!(
        !fs::read_dir(&dir.0).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "tmp"))
    );
}
#[test]
fn backup_failure_does_not_replace_original() {
    let dir = Directory::new();
    let path = dir.0.join("project.vvm.json");
    let mut p = project();
    let stamp = save_document(&path, &p, None).unwrap();
    let original = fs::read(&path).unwrap();
    fs::create_dir(dir.0.join("project.vvm.json.bak")).unwrap();
    p.title = "not committed".into();
    assert!(save_document(&path, &p, Some(&stamp)).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}
#[test]
fn racing_new_saves_have_one_winner() {
    let dir = Directory::new();
    let path = dir.0.join("race.vvm.json");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                save_document(&path, &project(), None)
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    load_document(&path).unwrap();
}
#[test]
fn invalid_document_and_symlink_are_rejected_without_writes() {
    let dir = Directory::new();
    let path = dir.0.join("project.vvm.json");
    fs::write(&path, b"{}").unwrap();
    assert!(load_document(&path).is_err());
    let alias = dir.0.join("alias.vvm.json");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    assert!(save_document(&alias, &project(), None).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{}");
    assert!(save_document(&dir.0.join("wrong.json"), &project(), None).is_err());
}

#[test]
fn precise_positions_keep_project_hash_after_file_roundtrip() {
    use sungeki_project::{Clip, ClipContent, ShapeKind, TimeUs, Track, TrackKind, Transform};
    let dir = Directory::new();
    let path = dir.0.join("precise.vvm.json");
    let mut project = project();
    let track_id = uuid::Uuid::new_v4().to_string();
    project.tracks.push(Track {
        id: track_id.clone(),
        name: "精密配置".into(),
        kind: TrackKind::Visual,
        visible: true,
        muted: false,
    });
    for n in 1..256 {
        project.clips.push(Clip {
            id: uuid::Uuid::new_v4().to_string(),
            track_id: track_id.clone(),
            scene_id: None,
            start_us: TimeUs(0),
            duration_us: TimeUs(1_000_000),
            transform: Transform {
                motion: None,
                x: 1.0 + f64::from(n) / 997.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
                opacity: 1.0,
                fade: None,
                flip_x: None,
            },
            content: ClipContent::Shape {
                shape: ShapeKind::Rectangle,
                fill: "#123456".into(),
            },
        });
    }
    let hash = project_hash(&project).unwrap();
    save_document(&path, &project, None).unwrap();
    let loaded = load_document(&path).unwrap();
    assert_eq!(loaded.project, project);
    assert_eq!(project_hash(&loaded.project).unwrap(), hash);
}
