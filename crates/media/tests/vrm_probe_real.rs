#![cfg(unix)]
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{
    StopReason,
    vrm_probe::{VrmKind, VrmProbeConfig, VrmProbeError, probe_vrm},
};

#[test]
#[ignore = "requires pinned Node and local VRM/VRMA originals"]
fn metadata_worker_real_model_motion_and_cancel() {
    let node = PathBuf::from(std::env::var_os("VVM_TEST_NODE").expect("VVM_TEST_NODE"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let worker = root.join("workers/vrm/src/inspect.ts");
    let model = PathBuf::from(std::env::var_os("VVM_TEST_VRM").expect("VVM_TEST_VRM"));
    let motion = PathBuf::from(std::env::var_os("VVM_TEST_VRMA").expect("VVM_TEST_VRMA"));
    let config = VrmProbeConfig {
        node: &node,
        worker: &worker,
        directory: &root,
        timeout: Duration::from_secs(15),
    };
    let cancel = AtomicBool::new(false);
    let p = probe_vrm(&config, &model, VrmKind::Model, &cancel).unwrap();
    assert_eq!(p.inspection.kind, VrmKind::Model);
    assert!(p.inspection.expressions.contains(&"aa".into()));
    assert!(p.credit.is_some());
    let m = probe_vrm(&config, &motion, VrmKind::Motion, &cancel).unwrap();
    assert_eq!(m.inspection.kind, VrmKind::Motion);
    assert!(m.inspection.expressions.is_empty());
    assert_ne!(p.sha256, m.sha256);
    assert!(matches!(
        probe_vrm(&config, &motion, VrmKind::Model, &cancel),
        Err(VrmProbeError::Worker(_))
    ));
    assert!(matches!(
        probe_vrm(&config, &model, VrmKind::Model, &AtomicBool::new(true)),
        Err(VrmProbeError::Stopped(StopReason::Cancelled))
    ));
    println!("model={} motion={}", p.sha256, m.sha256);
}
