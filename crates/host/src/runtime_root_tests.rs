use super::*;

#[test]
fn configured_volume_does_not_depend_on_development_disk_name() {
    assert_eq!(
        mount_for(Path::new("/Volumes/動画用 SSD/Sungeki/runtime")).unwrap(),
        Path::new("/Volumes/動画用 SSD")
    );
    assert_eq!(
        mount_for(Path::new("/Volumes/TestSSD/Sungeki/runtime")).unwrap(),
        Path::new("/Volumes/TestSSD")
    );
    for path in [
        "",
        "relative/path",
        "/tmp/runtime",
        "/Volumes",
        "/Volumes/SSD",
        "/Volumes/SSD/../other/runtime",
    ] {
        assert!(mount_for(Path::new(path)).is_err(), "{path}");
    }
}

#[test]
fn unavailable_volume_does_not_create_fallback_directories() {
    let mount = Path::new("/Volumes").join(format!("vvm-absent-{}", uuid::Uuid::new_v4()));
    assert!(checked(&mount.join("runtime")).is_err());
    assert!(!mount.exists());
}

#[test]
#[ignore = "requires an explicitly configured external-disk QA root"]
fn external_runtime_and_symlink_escape() {
    let configured =
        PathBuf::from(std::env::var_os("SUNGEKI_RUNTIME_TEST_ROOT").expect("explicit QA root"));
    let base = checked(&configured).unwrap();
    let owned = base.join(format!("host-runtime-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&owned).unwrap();
    let directory = owned.join("runtime");
    fs::create_dir(&directory).unwrap();
    assert_eq!(checked(&directory).unwrap(), directory);
    std::os::unix::fs::symlink(std::env::temp_dir(), owned.join("escape")).unwrap();
    assert!(checked(&owned.join("escape")).is_err());
    let missing = owned.join("missing");
    let error = checked(&missing).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        error
            .to_string()
            .contains("既存フォルダを選び直してください")
    );
    assert!(error.to_string().contains(&missing.display().to_string()));
    assert!(!owned.join("missing").exists());
    fs::remove_dir_all(&owned).unwrap();
}

#[test]
fn saved_location_and_explicit_override_are_selected_without_fallback() {
    let saved = Path::new("/Volumes/My SSD/Sungeki/runtime");
    assert_eq!(selected(None, Some(saved)).unwrap(), saved);
    assert_eq!(
        selected(Some("/Volumes/Other/Sungeki".into()), Some(saved)).unwrap(),
        Path::new("/Volumes/Other/Sungeki")
    );
    assert_eq!(
        selected(Some("".into()), Some(saved)).unwrap(),
        PathBuf::new()
    );
    assert!(mount_for(&selected(Some("".into()), Some(saved)).unwrap()).is_err());
    let error = selected(None, None).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("保存フォルダが未設定"));
}
