use sungeki_project::*;
use sungeki_review::*;
fn project() -> Project {
    Project::empty(
        "00000000-0000-4000-8000-000000000001".into(),
        "確認レポート検証".into(),
    )
}
#[test]
fn valid_structure_does_not_claim_visual_or_asset_success() {
    let p = project();
    let original = p.clone();
    let report = review_structure(&p);
    assert_eq!(p, original);
    assert_eq!(
        report.summary,
        Summary {
            passed: 1,
            failed: 0,
            not_run: 5,
            issue_count: 0,
            complete: false
        }
    );
    assert_eq!(report.checks[0].status, CheckStatus::Passed);
    assert!(
        report.checks[1..]
            .iter()
            .all(|c| c.status == CheckStatus::NotRun)
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["checks"][1]["status"], "not_run");
}
#[test]
fn failure_reports_existing_validation_reason_without_panicking_or_losing_not_run() {
    let mut p = project();
    p.canvas.width = 0;
    let message = validate(&p).unwrap_err().0;
    let report = review_structure(&p);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.summary.not_run, 5);
    assert!(!report.summary.complete);
    assert_eq!(report.issues[0].message, message);
    assert_eq!(report.issues[0].code, "PROJECT_INVALID");
    assert_eq!(report.issues[0].clip_id, None);
}
#[test]
fn missing_references_are_not_treated_as_valid_structure() {
    let mut p = project();
    p.groups.push(Group {
        id: "00000000-0000-4000-8000-000000000002".into(),
        name: "参照切れ".into(),
        clip_ids: vec!["00000000-0000-4000-8000-000000000003".into()],
    });
    let original = p.clone();
    let report = review_structure(&p);
    assert_eq!(report.issues[0].message, validate(&p).unwrap_err().0);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(p, original);
}
