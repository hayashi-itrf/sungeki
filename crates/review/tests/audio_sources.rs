#![cfg(unix)]
use std::{fs, path::PathBuf};
use sungeki_project::*;
use sungeki_review::*;
fn fixture() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("review-audio-source-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(996u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(24000u32.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(960u32.to_le_bytes());
    bytes.extend(vec![0; 960]);
    let path = root.join("original.wav");
    fs::write(&path, &bytes).unwrap();
    let probe =
        sungeki_media::probe_audio(&path, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/audio-project.json")).unwrap();
    p.assets.truncate(1);
    p.clips.clear();
    p.assets[0].locator = AssetLocator::Absolute {
        path: path.to_string_lossy().into(),
    };
    p.assets[0].content_hash = Some(probe.sha256.clone());
    p.assets[0].probe = AssetProbe::Audio(probe);
    (root, p)
}
#[test]
fn valid_wav_is_read_without_modifying_the_source_or_project() {
    let (root, p) = fixture();
    let original = p.clone();
    let bytes = fs::read(root.join("original.wav")).unwrap();
    assert!(review_audio_sources(&p, 4096, || false).unwrap().is_empty());
    assert_eq!(p, original);
    assert_eq!(fs::read(root.join("original.wav")).unwrap(), bytes);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn changed_missing_and_symlinked_originals_are_identified_by_asset() {
    let (root, p) = fixture();
    let path = root.join("original.wav");
    let mut bytes = fs::read(&path).unwrap();
    bytes[50] = 1;
    fs::write(&path, &bytes).unwrap();
    let issues = review_audio_sources(&p, 4096, || false).unwrap();
    assert_eq!(issues[0].code, "AUDIO_SOURCE_CHANGED");
    assert_eq!(issues[0].asset_id.as_ref(), Some(&p.assets[0].id));
    fs::rename(&path, root.join("moved.wav")).unwrap();
    assert_eq!(
        review_audio_sources(&p, 4096, || false).unwrap()[0].code,
        "AUDIO_SOURCE_MISSING"
    );
    std::os::unix::fs::symlink(root.join("moved.wav"), &path).unwrap();
    assert_eq!(
        review_audio_sources(&p, 4096, || false).unwrap()[0].code,
        "AUDIO_SOURCE_UNREADABLE"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn altered_metadata_does_not_pass_just_because_hash_matches() {
    let (root, mut p) = fixture();
    let AssetProbe::Audio(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    probe.channels = 2;
    probe.frames = 240;
    probe.duration_us = TimeUs(10000);
    assert_eq!(
        review_audio_sources(&p, 4096, || false).unwrap()[0].code,
        "AUDIO_METADATA_MISMATCH"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn budget_and_cancellation_are_not_reported_as_successful_inspection() {
    let (root, p) = fixture();
    let issue = &review_audio_sources(&p, 1, || false).unwrap()[0];
    assert_eq!(issue.code, "AUDIO_SOURCE_NOT_INSPECTED");
    assert_eq!(issue.severity, Severity::Warning);
    assert!(matches!(
        review_audio_sources(&p, 4096, || true),
        Err(ImageReviewError::Cancelled)
    ));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn compressed_identity_check_does_not_claim_decoding_and_detects_growth() {
    // Arbitrary source bytes: this path verifies identity, never claims MP3 decoding.
    let (root, mut p) = fixture();
    let AssetProbe::Audio(probe) = &mut p.assets[0].probe else {
        panic!()
    };
    probe.encoding = Some(CompressedAudioEncoding::Mp3 {
        decode_version: 1,
        decoded_sha256: "a".repeat(64),
    });
    probe.sample_rate = 48000;
    probe.channels = 2;
    probe.frames = 480;
    probe.duration_us = TimeUs(10000);
    probe.validate().unwrap();
    assert!(review_audio_sources(&p, 4096, || false).unwrap().is_empty());
    let path = root.join("original.wav");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(0);
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        review_audio_sources(&p, 4096, || false).unwrap()[0].code,
        "AUDIO_SOURCE_CHANGED"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn verified_audio_baseline_tracks_byte_changes_without_claiming_full_review() {
    let (root, p) = fixture();
    let report = review_project_images(&p, &root.join("review"), 4096, || false).unwrap();
    let expected = report.asset_fingerprint.unwrap();
    assert_eq!(
        expected,
        current_source_fingerprint(&p.assets, 4096).unwrap()
    );
    assert!(!report.diagnostics.summary.complete);
    assert_eq!(
        report
            .diagnostics
            .checks
            .iter()
            .find(|c| c.kind == CheckKind::Audio)
            .unwrap()
            .status,
        CheckStatus::NotRun
    );
    let path = root.join("original.wav");
    let mut bytes = fs::read(&path).unwrap();
    bytes[50] ^= 1;
    fs::write(&path, bytes).unwrap();
    assert_ne!(
        expected,
        current_source_fingerprint(&p.assets, 4096).unwrap()
    );
    assert!(
        review_project_images(&p, &root.join("changed"), 4096, || false)
            .unwrap()
            .asset_fingerprint
            .is_none()
    );
    assert!(current_source_fingerprint(&p.assets, 1).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn mixed_images_audio_are_watchable_but_missing_psd_is_not() {
    let (root, mut p) = fixture();
    let image_path = root.join("image.png");
    fs::write(&image_path, include_bytes!("data/transparent.png")).unwrap();
    let probe = sungeki_assets::probe_image(&image_path, Default::default()).unwrap();
    let mut image = p.assets[0].clone();
    image.id = uuid::Uuid::new_v4().to_string();
    image.locator = AssetLocator::Absolute {
        path: image_path.to_string_lossy().into(),
    };
    image.content_hash = Some(probe.sha256.clone());
    image.probe = AssetProbe::Image(probe);
    p.assets.push(image);
    let report = review_project_images(&p, &root.join("mixed"), 4096, || false).unwrap();
    assert_eq!(
        report.asset_fingerprint.as_deref(),
        Some(
            current_source_fingerprint(&p.assets, 4096)
                .unwrap()
                .as_str()
        )
    );
    let psd: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    p.assets.push(psd.assets[0].clone());
    assert!(
        review_project_images(&p, &root.join("psd"), 4096, || false)
            .unwrap()
            .asset_fingerprint
            .is_none()
    );
    assert!(current_source_fingerprint(&p.assets, 4096).is_err());
    fs::remove_dir_all(root).unwrap();
}
