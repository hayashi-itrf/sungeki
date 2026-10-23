use super::*;
use std::{fs, os::unix::ffi::OsStringExt};

#[test]
fn environment_selection_keeps_saved_values_and_never_validates_targets() {
    let saved = Settings {
        format_version: 1,
        runtime_root: Some("/Volumes/未接続 disk/作業".into()),
        ffmpeg: Some("/not-installed/ffmpeg".into()),
        ffprobe: Some("/not-installed/ffprobe".into()),
        font: Some("/not-installed/日本語.ttf".into()),
        font_sha256: Some("a".repeat(64)),
        ..Default::default()
    };
    let database = Path::new("/the-connected-host/設定/recovery.sqlite");
    let report = describe(database, &saved, |name| match name {
        "SUNGEKI_RENDER_FFMPEG" => Some("relative-explicit-tool".into()),
        "SUNGEKI_RENDER_FFPROBE" => Some("".into()),
        "SUNGEKI_RENDER_FONT" => Some("/different/font.ttf".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(report.data_directory, database.parent().unwrap());
    assert_eq!(
        report.runtime_settings_path,
        database.parent().unwrap().join("runtime.json")
    );
    assert_eq!(
        report.saved.ffmpeg.as_deref(),
        Some("/not-installed/ffmpeg")
    );
    assert_eq!(
        report.selected.ffmpeg.value.as_deref(),
        Some("relative-explicit-tool")
    );
    assert_eq!(
        report.selected.ffmpeg.source,
        RuntimeSettingSource::Environment
    );
    assert_eq!(report.selected.ffprobe.value.as_deref(), Some(""));
    assert_eq!(
        report.selected.ffprobe.source,
        RuntimeSettingSource::Environment
    );
    assert_eq!(
        report.selected.font.value.as_deref(),
        Some("/different/font.ttf")
    );
    assert_eq!(report.selected.font_sha256.value, saved.font_sha256);
    assert_eq!(
        report.selected.font_sha256.source,
        RuntimeSettingSource::Saved
    );
    assert_eq!(
        report.selected.runtime_root.source,
        RuntimeSettingSource::Saved
    );
    assert_eq!(report.verification, RuntimeEnvironmentVerification::NotRun);
    assert_eq!(
        report.scope,
        RuntimeEnvironmentScope::RenderPathsAndRuntimeRoot
    );

    let report = describe(database, &saved, |name| {
        (name == "SUNGEKI_RENDER_FONT_SHA256").then(|| "invalid-explicit-hash".into())
    })
    .unwrap();
    assert_eq!(
        report.selected.font_sha256.value.as_deref(),
        Some("invalid-explicit-hash")
    );
    assert_eq!(
        report.selected.font_sha256.source,
        RuntimeSettingSource::Environment
    );
    assert_eq!(report.selected.font.source, RuntimeSettingSource::Saved);
}

#[test]
fn environment_reports_every_override_including_empty_and_does_not_fall_back() {
    let saved = Settings {
        format_version: 1,
        ..Default::default()
    };
    let report = describe(Path::new("/profile/recovery.sqlite"), &saved, |_| {
        Some("".into())
    })
    .unwrap();
    for selection in [
        &report.selected.runtime_root,
        &report.selected.ffmpeg,
        &report.selected.ffprobe,
        &report.selected.font,
        &report.selected.font_sha256,
    ] {
        assert_eq!(selection.source, RuntimeSettingSource::Environment);
        assert_eq!(selection.value.as_deref(), Some(""));
        assert!(selection.environment_variable.starts_with("SUNGEKI_"));
    }
    // The same selection used by production still rejects explicit bad paths.
    assert!(super::super::file(Some("".into()), Some(Path::new("/saved")), "ffmpeg").is_err());
    assert_eq!(
        crate::runtime_root::selected(Some("".into()), Some(Path::new("/saved"))).unwrap(),
        Path::new("")
    );
}

#[test]
fn environment_unset_does_not_create_settings_or_change_saved_bytes() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let database = fixture.0.join("host.sqlite");
    let settings_path = fixture.0.join("runtime.json");
    let before: Vec<_> = fs::read_dir(&fixture.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let saved = load(&database).unwrap();
    let report = describe(&database, &saved, |_| None).unwrap();
    for selection in [
        &report.selected.runtime_root,
        &report.selected.ffmpeg,
        &report.selected.ffprobe,
        &report.selected.font,
        &report.selected.font_sha256,
    ] {
        assert_eq!(selection.source, RuntimeSettingSource::Unset);
        assert_eq!(selection.value, None);
    }
    assert!(!settings_path.exists());
    assert_eq!(
        fs::read_dir(&fixture.0)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>(),
        before
    );
    let bytes = serde_json::to_vec(&saved).unwrap();
    fs::write(&settings_path, &bytes).unwrap();
    describe(&database, &load(&database).unwrap(), |_| None).unwrap();
    assert_eq!(fs::read(settings_path).unwrap(), bytes);
}

#[test]
fn environment_rejects_non_utf8_overrides_without_lossy_paths() {
    let error = describe(
        Path::new("/profile/recovery.sqlite"),
        &Settings::default(),
        |name| (name == "SUNGEKI_RENDER_FFMPEG").then(|| OsString::from_vec(vec![0xff])),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ValidationFailed);
    assert_eq!(error.details["reason"], "runtime_value_encoding");
    assert_eq!(
        error.details["environment_variable"],
        "SUNGEKI_RENDER_FFMPEG"
    );
}
