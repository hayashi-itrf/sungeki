#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use sungeki_project::*;
use sungeki_review::*;

fn fixture() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("review-video-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    // Byte identity fixture, deliberately not a VIDEO decoding test.
    let bytes = vec![42u8; 4096];
    let path = root.join("source.mp4");
    fs::write(&path, &bytes).unwrap();
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    p.clips.clear();
    let asset = &mut p.assets[0];
    asset.locator = AssetLocator::Absolute {
        path: path.to_string_lossy().into(),
    };
    let AssetProbe::Video(probe) = &mut asset.probe else {
        panic!()
    };
    probe.sha256 = format!("{:x}", Sha256::digest(&bytes));
    probe.byte_length = bytes.len() as u64;
    asset.content_hash = Some(probe.sha256.clone());
    (root, p)
}

#[test]
fn verified_bytes_are_watchable_without_claiming_decoding_or_visual_validation() {
    let (root, p) = fixture();
    let original = p.clone();
    assert!(review_video_sources(&p, 4096, || false).unwrap().is_empty());
    let report = review_project_images(&p, &root.join("review"), 4096, || false).unwrap();
    assert_eq!(
        report.asset_fingerprint,
        Some(current_source_fingerprint(&p.assets, 4096).unwrap())
    );
    assert!(!report.diagnostics.summary.complete);
    let check = report
        .diagnostics
        .checks
        .iter()
        .find(|c| c.kind == CheckKind::AssetIntegrity)
        .unwrap();
    assert_eq!(check.status, CheckStatus::NotRun);
    assert!(
        check
            .description
            .contains("映像・内蔵音声の復号、時刻情報、再生品質は未検査")
    );
    assert_eq!(p, original);
    assert_eq!(fs::read(root.join("source.mp4")).unwrap(), vec![42; 4096]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changes_missing_symlinks_and_growth_produce_targeted_diagnostics() {
    let (root, p) = fixture();
    let path = root.join("source.mp4");
    let baseline = current_source_fingerprint(&p.assets, 4096).unwrap();
    fs::write(&path, vec![43; 4096]).unwrap();
    assert_ne!(
        baseline,
        current_source_fingerprint(&p.assets, 4096).unwrap()
    );
    let report = review_project_images(&p, &root.join("changed"), 4096, || false).unwrap();
    assert!(report.asset_fingerprint.is_none());
    assert!(
        report
            .diagnostics
            .checks
            .iter()
            .any(|c| c.kind == CheckKind::AssetIntegrity && c.status == CheckStatus::Failed)
    );
    let issue = report
        .diagnostics
        .issues
        .iter()
        .find(|i| i.code == "VIDEO_SOURCE_CHANGED")
        .unwrap();
    assert_eq!(issue.asset_id.as_ref(), Some(&p.assets[0].id));
    fs::write(&path, vec![42; 4097]).unwrap();
    assert_eq!(
        review_video_sources(&p, 4096, || false).unwrap()[0].code,
        "VIDEO_SOURCE_CHANGED"
    );
    fs::rename(&path, root.join("moved.mp4")).unwrap();
    assert_eq!(
        review_video_sources(&p, 4096, || false).unwrap()[0].code,
        "VIDEO_SOURCE_MISSING"
    );
    std::os::unix::fs::symlink(root.join("moved.mp4"), &path).unwrap();
    assert_eq!(
        review_video_sources(&p, 4096, || false).unwrap()[0].code,
        "VIDEO_SOURCE_UNREADABLE"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn budget_skip_and_cancellation_never_create_a_baseline() {
    let (root, p) = fixture();
    let report = review_project_images(&p, &root.join("budget"), 4095, || false).unwrap();
    assert!(report.asset_fingerprint.is_none());
    assert!(
        report
            .diagnostics
            .issues
            .iter()
            .any(|i| i.code == "VIDEO_SOURCE_NOT_INSPECTED" && i.severity == Severity::Warning)
    );
    assert!(matches!(
        review_video_sources(&p, 4096, || true),
        Err(ImageReviewError::Cancelled)
    ));
    assert!(current_source_fingerprint(&p.assets, 4095).is_err());
    fs::remove_dir_all(root).unwrap();
}
