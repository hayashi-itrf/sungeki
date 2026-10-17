use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use sungeki_assets::{AssetLocationError as Error, AssetRoots, rebase, resolve};
use sungeki_project::AssetLocator as Locator;
fn relative(path: &str) -> Locator {
    Locator::ProjectRelative { path: path.into() }
}
fn roots(path: &Path) -> AssetRoots {
    AssetRoots {
        document_path: Some(path.into()),
        libraries: BTreeMap::new(),
    }
}

#[test]
fn relative_references_require_explicit_document_root() {
    assert_eq!(
        resolve(&relative("画像.png"), &AssetRoots::default()),
        Err(Error::UnsavedProject)
    );
    let base = std::env::temp_dir().join("作品/動画.vvm.json");
    assert_eq!(
        resolve(&relative("素材/画像.png"), &roots(&base)).unwrap(),
        base.parent().unwrap().join("素材/画像.png")
    );
    assert_eq!(
        resolve(&relative("image.png"), &roots(Path::new("video.vvm.json"))),
        Err(Error::InvalidRoot)
    );
}

#[test]
fn missing_library_is_explicit_and_save_as_preserves_it() {
    let locator = Locator::Library {
        library_id: "characters".into(),
        path: "a.png".into(),
    };
    assert_eq!(
        resolve(&locator, &AssetRoots::default()),
        Err(Error::MissingLibrary("characters".into()))
    );
    let new = std::env::temp_dir().join("new.vvm.json");
    assert_eq!(
        rebase(&locator, &AssetRoots::default(), &new).unwrap(),
        locator
    );
    let base = std::env::temp_dir().join("library");
    let roots = AssetRoots {
        document_path: None,
        libraries: BTreeMap::from([("characters".into(), base.clone())]),
    };
    assert_eq!(resolve(&locator, &roots).unwrap(), base.join("a.png"));
}

#[test]
fn invalid_paths_never_fall_back_to_working_directory() {
    let base = std::env::temp_dir().join("video.vvm.json");
    for path in ["", " ", "a\0.png"] {
        assert_eq!(
            resolve(&relative(path), &roots(&base)),
            Err(Error::InvalidPath)
        );
    }
    assert_eq!(
        resolve(
            &Locator::Absolute {
                path: "relative.png".into()
            },
            &roots(&base)
        ),
        Err(Error::InvalidPath)
    );
    assert_eq!(
        resolve(&relative(base.to_str().unwrap()), &roots(&base)),
        Err(Error::InvalidPath)
    );
}

#[test]
fn save_as_preserves_absent_media_target_and_same_directory_locator() {
    let base = std::env::temp_dir().join("vvm-original/video.vvm.json");
    let root = roots(&base);
    let locator = relative("missing/image.png");
    let same = base.with_file_name("renamed.vvm.json");
    assert_eq!(rebase(&locator, &root, &same).unwrap(), locator);
    let other = std::env::temp_dir().join("vvm-other/copy.vvm.json");
    let moved = rebase(&locator, &root, &other).unwrap();
    assert_eq!(
        resolve(&moved, &roots(&other)).unwrap(),
        resolve(&locator, &root).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn symlink_parent_traversal_keeps_its_real_target_after_save_as() {
    use std::{fs, os::unix::fs::symlink};
    let dir: PathBuf = std::env::temp_dir().join(format!("vvm-locator-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(dir.join("project")).unwrap();
    fs::create_dir_all(dir.join("external/subdir")).unwrap();
    fs::create_dir_all(dir.join("other")).unwrap();
    fs::write(dir.join("external/image.png"), b"expected").unwrap();
    fs::write(dir.join("project/image.png"), b"wrong").unwrap();
    symlink(dir.join("external/subdir"), dir.join("project/link")).unwrap();
    let old = roots(&dir.join("project/video.vvm.json"));
    let locator = relative("link/../image.png");
    let new = dir.join("other/copy.vvm.json");
    let rebased = rebase(&locator, &old, &new).unwrap();
    assert!(matches!(rebased, Locator::ProjectRelative { .. }));
    let before = resolve(&locator, &old).unwrap();
    let after = resolve(&rebased, &roots(&new)).unwrap();
    assert_eq!(fs::read(before).unwrap(), b"expected");
    assert_eq!(fs::read(after).unwrap(), b"expected");
    fs::remove_dir_all(dir).unwrap();
}
