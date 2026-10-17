use super::*;
use crate::runtime_settings::Settings;
#[test]
fn saved_runtime_resolves_without_environment_and_rejects_bad_overrides() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let file = fixture.0.join("fixture");
    std::fs::write(&file, b"fixture").unwrap();
    let saved = Settings {
        format_version: 1,
        runtime_root: Some("/Volumes/Custom SSD/Sungeki/runtime".into()),
        python: Some(file.clone()),
        image_worker: Some(file.clone()),
        ffmpeg: Some(file.clone()),
        ffprobe: Some(file.clone()),
        font: Some(file.clone()),
        font_sha256: Some("a".repeat(64)),
        vrm_node: Some(file.clone()),
        vrm_render_worker: Some(file.clone()),
        ..Default::default()
    };
    let config = resolve(&saved, true, |_| None).unwrap();
    assert_eq!(config.ffmpeg, file);
    let vrm = config.vrm.unwrap();
    assert_eq!(vrm.worker, file);
    assert_eq!(
        vrm.work_root,
        PathBuf::from("/Volumes/Custom SSD/Sungeki/runtime/vrm-work")
    );
    assert_eq!(config.font_sha256, "a".repeat(64));
    let override_file = fixture.0.join("override");
    std::fs::write(&override_file, b"override").unwrap();
    let config = resolve(&saved, true, |name| {
        (name == "SUNGEKI_RENDER_FFMPEG").then(|| override_file.clone().into_os_string())
    })
    .unwrap();
    assert_eq!(config.ffmpeg, override_file);
    assert!(
        resolve(&saved, true, |name| (name == "SUNGEKI_RENDER_FFMPEG")
            .then(|| "missing".into()))
        .is_err()
    );
    assert!(
        resolve(&saved, true, |name| (name == "SUNGEKI_RENDER_FONT_SHA256")
            .then(|| "invalid".into()))
        .is_err()
    );
    std::fs::write(
        fixture.0.join("runtime.json"),
        serde_json::to_vec(&saved).unwrap(),
    )
    .unwrap();
    let reloaded = crate::runtime_settings::load(&fixture.0.join("host.sqlite")).unwrap();
    assert_eq!(resolve(&reloaded, true, |_| None).unwrap().python, file);
    let unset = Settings {
        runtime_root: None,
        ..saved
    };
    let error = resolve(&unset, true, |_| None).unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnavailable);
    assert!(error.message.contains("保存フォルダが未設定"));
}

#[test]
fn unfinished_vrm_setup_does_not_block_2d_but_is_rejected_for_visible_vrm() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let file = fixture.0.join("tool");
    std::fs::write(&file, b"tool").unwrap();
    let saved = Settings {
        format_version: 1,
        python: Some(file.clone()),
        image_worker: Some(file.clone()),
        ffmpeg: Some(file.clone()),
        ffprobe: Some(file.clone()),
        font: Some(file),
        font_sha256: Some("a".repeat(64)),
        vrm_render_worker: Some("/missing-worker".into()),
        ..Default::default()
    };
    assert!(resolve(&saved, false, |_| None).unwrap().vrm.is_none());
    assert!(resolve(&saved, true, |_| None).is_err());
    assert!(
        resolve(&saved, false, |name| (name == "SUNGEKI_RENDER_FFMPEG")
            .then(|| "/missing-ffmpeg".into()))
        .is_err()
    );
    let mut project: sungeki_project::Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap();
    assert!(needs_vrm(&project));
    for track in &mut project.tracks {
        track.visible = false;
    }
    assert!(!needs_vrm(&project));
}
