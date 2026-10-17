use std::{cell::Cell, fs, path::PathBuf};
use sungeki_assets::{ProbeLimits, probe_image};
use sungeki_project::{Asset, AssetLocator, CheckStatus, Project};
use sungeki_review::{ImageReviewError, review_images};

struct Fixture {
    root: PathBuf,
    project: Project,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("vvm-review-images-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("original.png");
        fs::write(&source, include_bytes!("data/transparent.png")).unwrap();
        let probe = probe_image(&source, ProbeLimits::default()).unwrap();
        let mut project = Project::empty(uuid::Uuid::new_v4().to_string(), "画像検査".into());
        project.assets.push(Asset {
            id: uuid::Uuid::new_v4().to_string(),
            name: "透過画像".into(),
            locator: AssetLocator::Absolute {
                path: source.to_str().unwrap().into(),
            },
            source: None,
            credit: "試験用".into(),
            content_hash: Some(probe.sha256.clone()),
            probe: sungeki_project::AssetProbe::Image(probe),
        });
        Self {
            root,
            project,
            source,
        }
    }
    fn job(&self) -> PathBuf {
        self.root.join("job")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn retains_captured_original_until_review_is_dropped_without_mutating_project() {
    let f = Fixture::new();
    let original = f.project.clone();
    let report = review_images(&f.project, &f.job(), 1024, || false).unwrap();
    assert_eq!(report.check.status, CheckStatus::Passed);
    assert!(report.issues.is_empty());
    assert_eq!(report.originals.len(), 1);
    fs::remove_file(&f.source).unwrap();
    assert_eq!(
        sungeki_project::AssetProbe::Image(
            probe_image(&report.originals[0].path, ProbeLimits::default()).unwrap()
        ),
        original.assets[0].probe
    );
    assert_eq!(f.project, original);
    assert!(f.job().exists());
    drop(report);
    assert!(!f.job().exists());
}

#[test]
fn distinguishes_missing_changed_and_inconsistent_registered_metadata() {
    let mut f = Fixture::new();
    for code in ["ASSET_METADATA_MISMATCH", "ASSET_CHANGED", "ASSET_MISSING"] {
        match code {
            "ASSET_METADATA_MISMATCH" => {
                if let sungeki_project::AssetProbe::Image(p) = &mut f.project.assets[0].probe {
                    p.width = 100;
                }
            }
            "ASSET_CHANGED" => fs::write(&f.source, b"changed").unwrap(),
            _ => fs::remove_file(&f.source).unwrap(),
        }
        let report = review_images(&f.project, &f.job(), 1024, || false).unwrap();
        assert_eq!(report.check.status, CheckStatus::Failed);
        assert_eq!(report.issues[0].code, code);
        assert_eq!(
            report.issues[0].asset_id.as_ref(),
            Some(&f.project.assets[0].id)
        );
        assert_eq!(report.issues[0].field, "/assets/0");
        assert!(report.originals.is_empty());
        drop(report);
        assert!(!f.job().exists());
    }
}

#[test]
fn grown_original_is_changed_without_expanding_the_read_budget() {
    let f = Fixture::new();
    let registered_size = fs::metadata(&f.source).unwrap().len();
    let mut bytes = fs::read(&f.source).unwrap();
    bytes.extend_from_slice(b"changed original");
    fs::write(&f.source, &bytes).unwrap();
    let before = f.project.clone();
    let report = review_images(&f.project, &f.job(), registered_size, || false).unwrap();
    assert_eq!(report.check.status, CheckStatus::Failed);
    assert_eq!(report.issues[0].code, "ASSET_CHANGED");
    assert_eq!(
        report.issues[0].asset_id.as_ref(),
        Some(&f.project.assets[0].id)
    );
    assert!(report.originals.is_empty());
    assert_eq!(fs::read_dir(f.job()).unwrap().count(), 0);
    assert_eq!(fs::read(&f.source).unwrap(), bytes);
    assert_eq!(f.project, before);
    drop(report);
    assert!(!f.job().exists());
}

#[test]
fn total_budget_applies_across_images_and_never_claims_pass_on_exhaustion() {
    let mut f = Fixture::new();
    let mut second = f.project.assets[0].clone();
    second.id = uuid::Uuid::new_v4().to_string();
    f.project.assets.push(second);
    let sungeki_project::AssetProbe::Image(probe) = &f.project.assets[0].probe else {
        panic!()
    };
    let limit = probe.byte_length;
    let report = review_images(&f.project, &f.job(), limit, || false).unwrap();
    assert_eq!(report.check.status, CheckStatus::Failed);
    assert_eq!(report.originals.len(), 1);
    assert_eq!(report.issues[0].code, "ASSET_CAPTURE_FAILED");
    assert_eq!(fs::read_dir(f.job()).unwrap().count(), 1);
}

#[test]
fn cancellation_after_capture_cleans_copies_and_returns_no_report() {
    let f = Fixture::new();
    let polls = Cell::new(0);
    let result = review_images(&f.project, &f.job(), 1024, || {
        let previous = polls.get();
        polls.set(previous + 1);
        previous > 0
    });
    assert!(matches!(result, Err(ImageReviewError::Cancelled)));
    assert_eq!(polls.get(), 2);
    assert!(!f.job().exists());
    assert!(f.source.exists());
}

#[test]
fn never_reuses_or_removes_a_preexisting_job_directory() {
    let f = Fixture::new();
    fs::create_dir(f.job()).unwrap();
    fs::write(f.job().join("keep"), b"existing job").unwrap();
    assert!(matches!(
        review_images(&f.project, &f.job(), 1024, || false),
        Err(ImageReviewError::Io(_))
    ));
    assert_eq!(fs::read(f.job().join("keep")).unwrap(), b"existing job");
}

#[test]
fn combined_report_counts_only_performed_checks_and_cleans_before_return() {
    let f = Fixture::new();
    let result =
        sungeki_review::review_project_images(&f.project, &f.job(), 1024, || false).unwrap();
    assert_eq!(result.diagnostics.summary.passed, 2);
    assert_eq!(result.diagnostics.summary.failed, 0);
    assert_eq!(result.diagnostics.summary.not_run, 4);
    assert!(!result.diagnostics.summary.complete);
    assert!(result.asset_fingerprint.is_some());
    assert!(!f.job().exists());
    fs::remove_file(&f.source).unwrap();
    let missing =
        sungeki_review::review_project_images(&f.project, &f.job(), 1024, || false).unwrap();
    assert_eq!(missing.diagnostics.summary.passed, 1);
    assert_eq!(missing.diagnostics.summary.failed, 1);
    assert_eq!(missing.diagnostics.summary.not_run, 4);
    assert_eq!(missing.diagnostics.issues[0].code, "ASSET_MISSING");
    assert!(missing.asset_fingerprint.is_none());
    assert!(!f.job().exists());
}

#[test]
fn fingerprint_is_order_independent_but_tracks_asset_identity() {
    let mut f = Fixture::new();
    let mut second = f.project.assets[0].clone();
    second.id = uuid::Uuid::new_v4().to_string();
    f.project.assets.push(second);
    let directory = f.job();
    let inspect = |p: &Project| {
        sungeki_review::review_project_images(p, &directory, 1024, || false)
            .unwrap()
            .asset_fingerprint
            .unwrap()
    };
    let initial = inspect(&f.project);
    f.project.assets.reverse();
    assert_eq!(inspect(&f.project), initial);
    f.project.assets[0].id = uuid::Uuid::new_v4().to_string();
    assert_ne!(inspect(&f.project), initial);
}

#[test]
fn invalid_structure_never_claims_asset_inspection() {
    let mut f = Fixture::new();
    f.project.canvas.width = 0;
    let report =
        sungeki_review::review_project_images(&f.project, &f.job(), 1024, || false).unwrap();
    assert_eq!(report.diagnostics.summary.failed, 1);
    assert_eq!(report.diagnostics.summary.not_run, 5);
    assert!(report.asset_fingerprint.is_none());
    assert!(!f.job().exists());
}

#[test]
fn cleanup_failure_is_reported_and_does_not_delete_a_replacement_file() {
    let f = Fixture::new();
    let mut result = review_images(&f.project, &f.job(), 1024, || false).unwrap();
    fs::remove_dir_all(f.job()).unwrap();
    fs::write(f.job(), b"replacement").unwrap();
    assert!(result.cleanup().is_err());
    assert_eq!(fs::read(f.job()).unwrap(), b"replacement");
    fs::remove_file(f.job()).unwrap();
    result.cleanup().unwrap();
    result.cleanup().unwrap();
}

#[test]
fn audio_is_not_reported_as_inspected_image() {
    let mut f = Fixture::new();
    let mut audio = f.project.assets[0].clone();
    audio.id = uuid::Uuid::new_v4().to_string();
    audio.probe = sungeki_project::AssetProbe::Audio(sungeki_project::AudioProbe {
        encoding: None,
        sha256: "a".repeat(64),
        duration_us: sungeki_project::TimeUs(10_000),
        sample_rate: 24000,
        channels: 1,
        frames: 240,
        byte_length: 524,
    });
    audio.content_hash = Some("a".repeat(64));
    f.project.assets.push(audio);
    let result =
        sungeki_review::review_project_images(&f.project, &f.job(), 1024, || false).unwrap();
    let check = result
        .diagnostics
        .checks
        .iter()
        .find(|c| c.kind == sungeki_review::CheckKind::AssetIntegrity)
        .unwrap();
    assert_eq!(check.status, CheckStatus::NotRun);
    assert!(result.asset_fingerprint.is_none());
    assert!(!f.job().exists());
}

#[test]
fn failed_capture_cannot_reuse_its_reserved_budget_for_the_next_image() {
    let mut f = Fixture::new();
    let mut second = f.project.assets[0].clone();
    second.id = uuid::Uuid::new_v4().to_string();
    // The second source exists; the first source is absent.
    f.project.assets[0].locator = AssetLocator::Absolute {
        path: f.root.join("missing.png").to_string_lossy().into_owned(),
    };
    let size = fs::metadata(&f.source).unwrap().len();
    f.project.assets.push(second);
    let report = review_images(&f.project, &f.job(), size, || false).unwrap();
    assert!(report.originals.is_empty());
    assert_eq!(report.issues[0].code, "ASSET_MISSING");
    assert_eq!(report.issues[1].code, "ASSET_CAPTURE_FAILED");
    assert_eq!(
        report.issues[1].asset_id.as_ref(),
        Some(&f.project.assets[1].id)
    );
}
