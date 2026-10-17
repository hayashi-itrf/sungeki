#![cfg(unix)]
//! Byte-identity fixtures, not evidence of PSD/video decoding.
use sha2::{Digest, Sha256};
use std::fs;
use sungeki_project::*;
use sungeki_review::*;

#[test]
fn psd_and_video_share_one_scan_budget_and_skips_are_explicit() {
    let root = std::env::temp_dir().join(format!("review-budget-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
    project.clips.clear();
    project.characters.clear();
    let video: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    project.assets.push(video.assets[0].clone());
    let bytes = vec![42u8; 4096];
    let hash = format!("{:x}", Sha256::digest(&bytes));
    for (i, asset) in project.assets.iter_mut().enumerate() {
        asset.id = uuid::Uuid::new_v4().to_string();
        let path = root.join(format!("source-{i}"));
        fs::write(&path, &bytes).unwrap();
        asset.locator = AssetLocator::Absolute {
            path: path.to_string_lossy().into(),
        };
        asset.content_hash = Some(hash.clone());
        match &mut asset.probe {
            AssetProbe::Psd(p) => {
                p.sha256 = hash.clone();
                p.byte_length = 4096;
            }
            AssetProbe::Video(p) => {
                p.sha256 = hash.clone();
                p.byte_length = 4096;
            }
            _ => panic!(),
        }
    }
    let complete = review_project_images(&project, &root.join("complete"), 8192, || false).unwrap();
    assert_eq!(
        complete.asset_fingerprint,
        Some(current_source_fingerprint(&project.assets, 8192).unwrap())
    );
    // Missing source must not be opened after the PSD consumed the entire budget.
    fs::remove_file(root.join("source-1")).unwrap();
    let limited = review_project_images(&project, &root.join("limited"), 4096, || false).unwrap();
    assert!(limited.asset_fingerprint.is_none());
    let issues = &limited.diagnostics.issues;
    assert!(issues.iter().any(|i| i.code == "VIDEO_SOURCE_NOT_INSPECTED"
        && i.asset_id.as_ref() == Some(&project.assets[1].id)));
    assert!(!issues.iter().any(|i| i.code == "VIDEO_SOURCE_MISSING"));
    let enough = review_project_images(&project, &root.join("enough"), 8192, || false).unwrap();
    assert!(
        enough
            .diagnostics
            .issues
            .iter()
            .any(|i| i.code == "VIDEO_SOURCE_MISSING")
    );
    assert!(enough.asset_fingerprint.is_none());
    fs::remove_dir_all(root).unwrap();
}
