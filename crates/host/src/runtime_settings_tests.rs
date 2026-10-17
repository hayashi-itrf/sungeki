use super::*;
use std::fs;
#[test]
fn saved_settings_are_isolated_bounded_and_do_not_hide_invalid_overrides() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let database = fixture.0.join("host.sqlite");
    let path = fixture.0.join("runtime.json");
    assert_eq!(load(&database).unwrap().format_version, 1);
    let binary = fixture.0.join("ffmpeg");
    fs::write(&binary, b"fixture").unwrap();
    let settings = Settings {
        format_version: 1,
        ffmpeg: Some(binary.clone()),
        ..Settings::default()
    };
    let bytes = serde_json::to_vec(&settings).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(load(&database).unwrap(), settings);
    assert_eq!(
        file(None, settings.ffmpeg.as_deref(), "FFmpeg").unwrap(),
        binary
    );
    assert!(
        file(
            Some(fixture.0.join("missing")),
            settings.ffmpeg.as_deref(),
            "FFmpeg"
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let other = fixture.0.join("other");
    fs::create_dir(&other).unwrap();
    assert!(load(&other.join("host.sqlite")).unwrap().ffmpeg.is_none());
    for invalid in [
        br#"{"format_version":2}"#.as_slice(),
        br#"{"format_version":1,"ffmpeg":"relative"}"#,
        br#"{"format_version":1,"unexpected":true}"#,
        br#"{"format_version":1,"font_sha256":"bad"}"#,
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(load(&database).is_err());
    }
    fs::write(&path, vec![b' '; LIMIT as usize + 1]).unwrap();
    assert!(load(&database).is_err());
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&binary, &path).unwrap();
    assert!(load(&database).is_err());
}
#[test]
fn save_is_atomic_private_and_rejects_stale_or_invalid_edits() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = crate::speech_journal_tests::Fixture::new();
    let database = fixture.0.join("host.sqlite");
    let expected = load(&database).unwrap();
    let mut settings = expected.clone();
    settings.ffmpeg = Some(fixture.0.join("ffmpeg"));
    let saved = store::save(&database, &expected, settings.clone()).unwrap();
    assert_eq!(load(&database).unwrap(), saved);
    assert_eq!(
        fs::metadata(fixture.0.join("runtime.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        store::save(&database, &expected, expected.clone())
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let mut bad = settings.clone();
    bad.ffmpeg = Some("relative".into());
    assert!(store::save(&database, &settings, bad).is_err());
    assert_eq!(load(&database).unwrap(), saved);
    assert!(
        fs::read_dir(&fixture.0).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
}
#[test]
fn common_api_retains_settings_after_reopen_and_rejects_stale_save() {
    use crate::sessions::Sessions;
    use std::sync::{Arc, Mutex};
    use sungeki_protocol::{SaveRuntimeSettingsArgs, SessionRequest, SessionResponse};
    let fixture = crate::speech_journal_tests::Fixture::new();
    let shared = Arc::new(Mutex::new(fixture.open()));
    let SessionResponse::RuntimeSettings { settings: expected } =
        Sessions::execute(&shared, SessionRequest::GetRuntimeSettings).unwrap()
    else {
        panic!("settings response")
    };
    let mut desired = expected.clone();
    desired.ffprobe = Some(fixture.0.join("ffprobe"));
    Sessions::execute(
        &shared,
        SessionRequest::SaveRuntimeSettings {
            args: SaveRuntimeSettingsArgs {
                expected: expected.clone(),
                settings: desired.clone(),
            },
        },
    )
    .unwrap();
    let stale = Sessions::execute(
        &shared,
        SessionRequest::SaveRuntimeSettings {
            args: SaveRuntimeSettingsArgs {
                expected,
                settings: desired.clone(),
            },
        },
    )
    .unwrap_err();
    assert_eq!(stale.code, ErrorCode::Conflict);
    drop(shared);
    let reopened = Arc::new(Mutex::new(fixture.open()));
    let SessionResponse::RuntimeSettings { settings } =
        Sessions::execute(&reopened, SessionRequest::GetRuntimeSettings).unwrap()
    else {
        panic!("settings response")
    };
    assert_eq!(settings, desired);
}

#[test]
fn font_hash_is_measured_by_common_save_without_mutating_supplied_pins() {
    use crate::sessions::Sessions;
    use std::sync::{Arc, Mutex};
    use sungeki_protocol::{SaveRuntimeSettingsArgs, SessionRequest, SessionResponse};
    let fixture = crate::speech_journal_tests::Fixture::new();
    let shared = Arc::new(Mutex::new(fixture.open()));
    let font = fixture.0.join("selected.ttf");
    fs::write(&font, b"abc").unwrap();
    let SessionResponse::RuntimeSettings { settings: expected } =
        Sessions::execute(&shared, SessionRequest::GetRuntimeSettings).unwrap()
    else {
        panic!()
    };
    let mut desired = expected.clone();
    desired.font = Some(font.clone());
    let SessionResponse::RuntimeSettings { settings: saved } = Sessions::execute(
        &shared,
        SessionRequest::SaveRuntimeSettings {
            args: SaveRuntimeSettingsArgs {
                expected: expected.clone(),
                settings: desired.clone(),
            },
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(
        saved.font_sha256.as_deref(),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
    assert_eq!(fs::read(&font).unwrap(), b"abc");
    fs::write(&font, b"changed").unwrap();
    let SessionResponse::RuntimeSettings { settings: pinned } = Sessions::execute(
        &shared,
        SessionRequest::SaveRuntimeSettings {
            args: SaveRuntimeSettingsArgs {
                expected: saved.clone(),
                settings: saved.clone(),
            },
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(pinned, saved); // Never silently bless changed bytes behind an existing pin.
    let stale = Sessions::execute(
        &shared,
        SessionRequest::SaveRuntimeSettings {
            args: SaveRuntimeSettingsArgs {
                expected,
                settings: desired,
            },
        },
    )
    .unwrap_err();
    assert_eq!(stale.code, ErrorCode::Conflict);
}

#[test]
fn automatic_font_hash_rejects_missing_empty_oversized_and_symlink_files() {
    let fixture = crate::speech_journal_tests::Fixture::new();
    let empty = fixture.0.join("empty");
    fs::write(&empty, []).unwrap();
    let large = fixture.0.join("large");
    fs::File::create(&large)
        .unwrap()
        .set_len(128 * 1024 * 1024 + 1)
        .unwrap();
    let link = fixture.0.join("link");
    std::os::unix::fs::symlink(&empty, &link).unwrap();
    for path in [
        empty,
        large,
        link,
        fixture.0.join("missing"),
        fixture.0.clone(),
    ] {
        let mut settings = Settings {
            format_version: 1,
            font: Some(path),
            ..Settings::default()
        };
        assert!(font::complete(&mut settings).is_err());
        assert!(settings.font_sha256.is_none());
    }
}

#[test]
fn storage_setting_roundtrip_and_active_job_guard() {
    use std::sync::atomic::Ordering;
    let fixture = crate::speech_journal_tests::Fixture::new();
    let sessions = fixture.open();
    let expected = load(&sessions.review_database_path).unwrap();
    let mut changed = expected.clone();
    changed.runtime_root = Some("/Volumes/Other SSD/Sungeki/runtime".into());
    validate(&changed).unwrap();
    for active in [
        &sessions.speech_runtime.active,
        &sessions.review_runtime.active,
        &sessions.build_runtime.active,
        &sessions.render_runtime.active,
        &sessions.loudness_runtime.active,
    ] {
        active.store(1, Ordering::SeqCst);
        assert_eq!(
            ensure_storage_idle(&sessions, &changed).unwrap_err().code,
            ErrorCode::Conflict
        );
        ensure_storage_idle(&sessions, &expected).unwrap();
        active.store(0, Ordering::SeqCst);
    }
    let prep = sessions
        .speech_runtime
        .preparations
        .claim(&uuid::Uuid::new_v4().to_string(), "session", "hash")
        .unwrap();
    assert_eq!(
        ensure_storage_idle(&sessions, &changed).unwrap_err().code,
        ErrorCode::Conflict
    );
    drop(prep);
    ensure_storage_idle(&sessions, &changed).unwrap();
    store::save(&sessions.review_database_path, &expected, changed.clone()).unwrap();
    assert_eq!(load(&sessions.review_database_path).unwrap(), changed);
    changed.runtime_root = Some("/tmp/runtime".into());
    assert!(validate(&changed).is_err());
}
