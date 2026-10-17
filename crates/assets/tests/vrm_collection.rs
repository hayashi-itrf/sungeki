#![cfg(unix)]
use sha2::{Digest, Sha256};
use sungeki_assets::{CollectionInput, CollectionLimits, plan_collection};
use sungeki_project::{AssetLocator, AssetProbe, Project};
#[test]
fn registered_model_and_motion_keep_distinct_extensions_and_source_hashes() {
    let root = std::env::temp_dir().join(format!("vrm-collect-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-assets-project.json"
    ))
    .unwrap();
    for (i, asset) in p.assets.iter_mut().enumerate() {
        // Collection treats content as opaque; GLB/GPU validation is tested separately.
        let bytes = vec![i as u8; 100];
        let path = root.join(i.to_string());
        std::fs::write(&path, &bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        asset.content_hash = Some(hash.clone());
        let AssetProbe::Vrm(probe) = &mut asset.probe else {
            panic!()
        };
        probe.sha256 = hash;
        asset.locator = AssetLocator::Absolute {
            path: path.to_string_lossy().into_owned(),
        };
    }
    let before = p.clone();
    let plan = plan_collection(
        &[CollectionInput { project: &p }],
        CollectionLimits {
            max_file_bytes: 1024,
            max_scan_bytes: 2048,
        },
        || false,
    )
    .unwrap();
    assert!(plan.sources_ready());
    assert_eq!(plan.files.len(), 2);
    assert!(
        plan.destination_for(0, &p.assets[0].id)
            .unwrap()
            .ends_with(".vrm")
    );
    assert!(
        plan.destination_for(0, &p.assets[1].id)
            .unwrap()
            .ends_with(".vrma")
    );
    assert_eq!(plan.estimated_asset_bytes, 200);
    assert_eq!(p, before);
    std::fs::remove_dir_all(root).unwrap();
}
