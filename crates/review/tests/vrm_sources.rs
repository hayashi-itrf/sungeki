#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use sungeki_project::*;
use sungeki_review::*;

fn fixture() -> (PathBuf, Project) {
    let root = std::env::temp_dir().join(format!("review-vrm-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    // Byte identity fixture, deliberately not a VRM parsing or rendering test.
    let bytes = vec![42u8; 4096];
    let path = root.join("source.vrm");
    fs::write(&path, &bytes).unwrap();
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-assets-project.json"
    ))
    .unwrap();
    p.clips.clear();
    p.assets.truncate(1);
    let asset = &mut p.assets[0];
    asset.locator = AssetLocator::Absolute {
        path: path.to_string_lossy().into(),
    };
    let AssetProbe::Vrm(probe) = &mut asset.probe else {
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
    assert!(review_vrm_sources(&p, 4096, || false).unwrap().is_empty());
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
            .contains("モデル・モーションの再解析と描画は未検査")
    );
    assert_eq!(p, original);
    assert_eq!(fs::read(root.join("source.vrm")).unwrap(), vec![42; 4096]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changes_missing_symlinks_and_growth_produce_targeted_diagnostics() {
    let (root, p) = fixture();
    let path = root.join("source.vrm");
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
        .find(|i| i.code == "VRM_SOURCE_CHANGED")
        .unwrap();
    assert_eq!(issue.asset_id.as_ref(), Some(&p.assets[0].id));
    fs::write(&path, vec![42; 4097]).unwrap();
    assert_eq!(
        review_vrm_sources(&p, 4096, || false).unwrap()[0].code,
        "VRM_SOURCE_CHANGED"
    );
    fs::rename(&path, root.join("moved.vrm")).unwrap();
    assert_eq!(
        review_vrm_sources(&p, 4096, || false).unwrap()[0].code,
        "VRM_SOURCE_MISSING"
    );
    std::os::unix::fs::symlink(root.join("moved.vrm"), &path).unwrap();
    assert_eq!(
        review_vrm_sources(&p, 4096, || false).unwrap()[0].code,
        "VRM_SOURCE_UNREADABLE"
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
            .any(|i| i.code == "VRM_SOURCE_NOT_INSPECTED" && i.severity == Severity::Warning)
    );
    assert!(matches!(
        review_vrm_sources(&p, 4096, || true),
        Err(ImageReviewError::Cancelled)
    ));
    assert!(current_source_fingerprint(&p.assets, 4095).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn image_success_cannot_mask_missing_or_budget_skipped_motion() {
    let (root, mut project) = fixture();
    if let AssetProbe::Vrm(probe) = &mut project.assets[0].probe {
        probe.inspection.kind = VrmKind::Motion;
        probe.inspection.expressions.clear();
    }
    let source = root.join("image.png");
    fs::write(&source, include_bytes!("data/transparent.png")).unwrap();
    let probe =
        sungeki_assets::probe_image(&source, sungeki_assets::ProbeLimits::default()).unwrap();
    let image_bytes = probe.byte_length;
    project.assets.push(Asset {
        id: uuid::Uuid::new_v4().to_string(),
        name: "image".into(),
        source: None,
        credit: "test".into(),
        locator: AssetLocator::Absolute {
            path: source.to_string_lossy().into(),
        },
        content_hash: Some(probe.sha256.clone()),
        probe: AssetProbe::Image(probe),
    });
    let skipped =
        review_project_images(&project, &root.join("skip"), image_bytes, || false).unwrap();
    assert!(skipped.asset_fingerprint.is_none());
    assert!(
        skipped
            .diagnostics
            .issues
            .iter()
            .any(|i| i.code == "VRM_SOURCE_NOT_INSPECTED")
    );
    let ready =
        review_project_images(&project, &root.join("ready"), image_bytes + 4096, || false).unwrap();
    assert_eq!(
        ready.asset_fingerprint,
        Some(current_source_fingerprint(&project.assets, image_bytes + 4096).unwrap())
    );
    fs::remove_file(root.join("source.vrm")).unwrap();
    let missing =
        review_project_images(&project, &root.join("missing"), image_bytes + 4096, || {
            false
        })
        .unwrap();
    assert!(missing.asset_fingerprint.is_none());
    assert!(
        missing
            .diagnostics
            .issues
            .iter()
            .any(|i| i.code == "VRM_SOURCE_MISSING")
    );
    fs::remove_dir_all(root).unwrap();
}
