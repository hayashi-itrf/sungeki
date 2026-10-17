#![cfg(unix)]
use sungeki_project::{CheckKind, CheckStatus, Project};
#[test]
fn vrm_registration_never_claims_a_verified_baseline_or_render() {
    let p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-assets-project.json"
    ))
    .unwrap();
    let root = std::env::temp_dir().join(format!("vrm-review-{}", uuid::Uuid::new_v4()));
    let report = sungeki_review::review_project_images(&p, &root, 1024 * 1024, || false).unwrap();
    assert_eq!(report.asset_fingerprint, None);
    assert_eq!(
        report
            .diagnostics
            .checks
            .iter()
            .find(|c| c.kind == CheckKind::AssetIntegrity)
            .unwrap()
            .status,
        CheckStatus::Failed
    );
    assert_eq!(
        report
            .diagnostics
            .issues
            .iter()
            .filter(|i| i.code == "VRM_SOURCE_MISSING")
            .count(),
        2
    );
    assert!(!root.exists());
}
