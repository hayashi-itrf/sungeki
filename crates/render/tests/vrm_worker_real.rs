#![cfg(unix)]
use std::{path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_render::{
    vrm_request::VrmRenderRequest,
    vrm_worker::{VrmWorkerConfig, render_vrm_frames},
};
#[test]
#[ignore = "requires local VRM/VRMA, pinned Node/Chromium and GPU"]
fn captures_real_vrm_images_with_time_and_expression_identity() {
    let node = PathBuf::from(std::env::var_os("VVM_TEST_NODE").expect("node"));
    let request = PathBuf::from(std::env::var_os("VVM_TEST_VRM_REQUEST").expect("request"));
    let output = PathBuf::from(std::env::var_os("VVM_TEST_VRM_OUTPUT").expect("output"));
    let root = PathBuf::from(std::env::var_os("VVM_TEST_VRM_WORK_ROOT").expect("work root"));
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let worker = repo.join("workers/vrm/src/render-main.ts");
    let config = VrmWorkerConfig {
        node: &node,
        worker: &worker,
        work_root: &root,
        timeout: Duration::from_secs(180),
    };
    let request: VrmRenderRequest =
        serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
    std::fs::create_dir(&output).unwrap();
    let result = render_vrm_frames(&request, &config, &output, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.len(), 6);
    assert_eq!(result[1].probe.sha256, result[5].probe.sha256);
    assert_ne!(result[1].probe.sha256, result[2].probe.sha256);
    assert!(result.iter().all(|f| f.probe.width == 640
        && f.probe.height == 360
        && f.probe.has_alpha
        && f.path.starts_with(&output)));
    println!(
        "captured {} verified PNGs in {}",
        result.len(),
        output.display()
    );
}
