#![cfg(unix)]
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::{fs, io::Cursor, path::PathBuf};
use sungeki_assets::*;
use sungeki_project::*;
struct Fixture {
    root: PathBuf,
    project: Project,
    original: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("collect-plan-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let mut buffer = Cursor::new(vec![]);
        DynamicImage::ImageRgba8(RgbaImage::new(8, 5))
            .write_to(&mut buffer, ImageFormat::Png)
            .unwrap();
        let original = buffer.into_inner();
        let path = root.join("原本.png");
        fs::write(&path, &original).unwrap();
        let probe = probe_image(&path, ProbeLimits::default()).unwrap();
        let mut project = Project::empty(uuid::Uuid::new_v4().to_string(), "収集計画".into());
        project.assets.push(Asset {
            id: uuid::Uuid::new_v4().to_string(),
            name: "検証画像".into(),
            locator: AssetLocator::Absolute {
                path: path.to_str().unwrap().into(),
            },
            content_hash: Some(probe.sha256.clone()),
            probe: AssetProbe::Image(probe),
            source: Some("source attribution".into()),
            credit: "作者・ライセンス不明".into(),
        });
        Self {
            root,
            project,
            original,
        }
    }
    fn plan(&self, limit: u64) -> CollectionPlan {
        plan_collection(
            &[CollectionInput {
                project: &self.project,
            }],
            CollectionLimits {
                max_file_bytes: limit,
                max_scan_bytes: limit,
            },
            || false,
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn plans_relative_copies_without_touching_sources_or_projects_and_preserves_credits() {
    let f = Fixture::new();
    let before = f.project.clone();
    let plan = f.plan(1_000_000);
    assert!(plan.sources_ready());
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.estimated_asset_bytes, f.original.len() as u64);
    assert_eq!(
        plan.destination_for(0, &f.project.assets[0].id),
        Some(plan.files[0].relative_path.as_str())
    );
    assert_eq!(f.project, before);
    assert_eq!(fs::read(f.root.join("原本.png")).unwrap(), f.original);
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 1); // dry run creates nothing
}
#[test]
fn deduplicates_across_snapshots_and_can_use_another_verified_copy() {
    let f = Fixture::new();
    let mut older = f.project.clone();
    older.assets[0].locator = AssetLocator::Absolute {
        path: f.root.join("missing.png").to_str().unwrap().into(),
    };
    let plan = plan_collection(
        &[
            CollectionInput { project: &older },
            CollectionInput {
                project: &f.project,
            },
        ],
        Default::default(),
        || false,
    )
    .unwrap();
    assert!(plan.sources_ready());
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.sources.len(), 2);
    assert_eq!(plan.sources[0].status, CollectionSourceStatus::Missing);
    assert_eq!(plan.sources[1].status, CollectionSourceStatus::Verified);
    assert_eq!(
        plan.destination_for(0, &older.assets[0].id),
        plan.destination_for(1, &f.project.assets[0].id)
    );
}
#[test]
fn changed_missing_symlink_budget_and_cancel_are_not_success() {
    let f = Fixture::new();
    let path = f.root.join("原本.png");
    let skipped = f.plan(1);
    assert!(!skipped.sources_ready());
    assert_eq!(
        skipped.sources[0].status,
        CollectionSourceStatus::NotInspected
    );
    let mut altered = f.original.clone();
    altered[20] ^= 1;
    fs::write(&path, altered).unwrap();
    let changed = f.plan(1_000_000);
    assert_eq!(changed.sources[0].status, CollectionSourceStatus::Changed);
    assert!(!changed.sources_ready());
    fs::rename(&path, f.root.join("moved.png")).unwrap();
    assert_eq!(
        f.plan(1_000_000).sources[0].status,
        CollectionSourceStatus::Missing
    );
    std::os::unix::fs::symlink(f.root.join("moved.png"), &path).unwrap();
    assert!(matches!(
        f.plan(1_000_000).sources[0].status,
        CollectionSourceStatus::Unreadable(_)
    ));
    assert_eq!(
        plan_collection(
            &[CollectionInput {
                project: &f.project,
            }],
            Default::default(),
            || true
        )
        .unwrap_err(),
        CollectionError::Cancelled
    );
}
#[test]
fn relative_documents_are_not_accepted_before_portable_loading_is_connected() {
    let mut f = Fixture::new();
    f.project.assets[0].locator = AssetLocator::ProjectRelative {
        path: "原本.png".into(),
    };
    assert!(matches!(
        plan_collection(
            &[CollectionInput {
                project: &f.project
            }],
            Default::default(),
            || false
        ),
        Err(CollectionError::Invalid(_))
    ));
}
#[test]
fn inconsistent_metadata_is_reported_and_not_silently_deduplicated() {
    let mut f = Fixture::new();
    let mut second = f.project.assets[0].clone();
    second.id = uuid::Uuid::new_v4().to_string();
    let AssetProbe::Image(p) = &mut second.probe else {
        panic!()
    };
    p.width += 1;
    f.project.assets.push(second);
    let plan = f.plan(1_000_000);
    assert!(!plan.sources_ready());
    assert!(plan.files[0].conflicting_metadata);
    assert!(!plan.sources_ready());
    f.project.assets.pop();
    f.project.assets[0].content_hash = None;
    assert!(matches!(
        plan_collection(
            &[CollectionInput {
                project: &f.project
            }],
            Default::default(),
            || false
        ),
        Err(CollectionError::Invalid(_))
    ));
}
