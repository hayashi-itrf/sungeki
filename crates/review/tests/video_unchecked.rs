use sungeki_project::{CheckKind, CheckStatus, Project};
#[test]
fn missing_registered_video_is_reported_without_verified_baseline() {
    let mut p: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    let root = std::env::temp_dir().join(format!("video-review-{}", uuid::Uuid::new_v4()));
    p.assets[0].locator = sungeki_project::AssetLocator::Absolute {
        path: root
            .with_extension("missing.mp4")
            .to_string_lossy()
            .into_owned(),
    };
    let report = sungeki_review::review_project_images(&p, &root, 1024 * 1024, || false).unwrap();
    assert_eq!(report.asset_fingerprint, None);
    let integrity = report
        .diagnostics
        .checks
        .iter()
        .find(|c| c.kind == CheckKind::AssetIntegrity)
        .unwrap();
    assert_eq!(integrity.status, CheckStatus::Failed);
    assert!(
        report
            .diagnostics
            .issues
            .iter()
            .any(|i| i.code == "VIDEO_SOURCE_MISSING"
                && i.asset_id.as_ref() == Some(&p.assets[0].id))
    );
    assert!(!root.exists());
}
