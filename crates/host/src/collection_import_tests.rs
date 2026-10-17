use super::*;
use crate::collection_execute::import;
fn bundle(f: &Fixture) -> PathBuf {
    let (old, _) = super::history_tests::historical(f);
    execute_with_history(
        &f.project,
        &[old],
        &f.root.join("package"),
        CollectionLimits::default(),
        &uuid::Uuid::new_v4().to_string(),
        || false,
        |_| Ok(()),
    )
    .unwrap()
    .destination
}
#[test]
fn verifies_actual_bundle_after_relocation_and_imports_without_originals() {
    let f = Fixture::new();
    let path = bundle(&f);
    let moved = f.root.join("moved");
    fs::rename(&path, &moved).unwrap();
    fs::remove_file(&f.source).unwrap();
    fs::remove_file(f.root.join("old.psd")).unwrap();
    let verified = import::verify(&moved.join("project.vvm.json"), || false).unwrap();
    let mut store = sungeki_storage::RecoveryStore::open(&f.root.join("db")).unwrap();
    let outcome = store
        .import_checkpoints(
            &verified.document.project.id,
            "verified",
            &verified.checkpoints,
        )
        .unwrap();
    assert_eq!(outcome.inserted, 1);
    let summaries = store
        .list_checkpoints(&verified.document.project.id, None, 10)
        .unwrap();
    assert_eq!(summaries.len(), 1);
    let c = store
        .checkpoint(&summaries[0].id)
        .unwrap()
        .unwrap()
        .checkpoint;
    assert!(c.project.assets.iter().all(|a| matches!(&a.locator, AssetLocator::Absolute { path } if Path::new(path).starts_with(&moved))));
    let verified = import::verify(&moved.join("project.vvm.json"), || false).unwrap();
    assert_eq!(
        store
            .import_checkpoints(
                &verified.document.project.id,
                "verified",
                &verified.checkpoints
            )
            .unwrap()
            .unchanged,
        1
    );
}
#[test]
fn historical_asset_change_missing_and_metadata_tampering_reject_import() {
    for mode in 0..4 {
        let f = Fixture::new();
        let path = bundle(&f);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap();
        let cp = path.join(manifest["checkpoints"][0]["path"].as_str().unwrap());
        let portable: serde_json::Value = serde_json::from_slice(&fs::read(&cp).unwrap()).unwrap();
        let asset = path.join(
            portable["project"]["assets"][0]["locator"]["path"]
                .as_str()
                .unwrap(),
        );
        match mode {
            0 => fs::remove_file(asset).unwrap(),
            1 => fs::write(asset, vec![5; 2048]).unwrap(),
            2 => fs::write(cp, b"{}").unwrap(),
            _ => {
                let mut m = manifest;
                m["project_sha256"] = "0".repeat(64).into();
                fs::write(path.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
            }
        }
        assert!(import::verify(&path.join("project.vvm.json"), || false).is_err());
        assert!(path.exists());
    }
}
#[test]
fn escaping_manifest_symlink_and_cancellation_are_rejected_without_writes() {
    for mode in 0..3 {
        let f = Fixture::new();
        let path = bundle(&f);
        match mode {
            0 => {
                let p = path.join("manifest.json");
                let mut m: serde_json::Value =
                    serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
                m["checkpoints"][0]["path"] = "../outside.json".into();
                fs::write(p, serde_json::to_vec(&m).unwrap()).unwrap();
            }
            1 => {
                fs::rename(path.join("checkpoints"), f.root.join("outside")).unwrap();
                std::os::unix::fs::symlink(f.root.join("outside"), path.join("checkpoints"))
                    .unwrap();
            }
            _ => {}
        }
        assert!(import::verify(&path.join("project.vvm.json"), || mode == 2).is_err());
        assert!(path.exists());
    }
}
