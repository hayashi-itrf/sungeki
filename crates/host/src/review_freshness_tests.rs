use super::*;
use std::{fs, path::PathBuf};
use sungeki_assets::{ProbeLimits, probe_image};
use sungeki_project::AssetLocator;

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    asset: Asset,
    expected: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("vvm-freshness-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("image.png");
        fs::write(
            &source,
            include_bytes!("../../review/tests/data/transparent.png"),
        )
        .unwrap();
        let probe = probe_image(&source, ProbeLimits::default()).unwrap();
        let asset = Asset {
            id: uuid::Uuid::new_v4().to_string(),
            name: "再確認".into(),
            locator: AssetLocator::Absolute {
                path: source.to_str().unwrap().into(),
            },
            source: None,
            credit: "試験用".into(),
            content_hash: Some(probe.sha256.clone()),
            probe: sungeki_project::AssetProbe::Image(probe),
        };
        let expected =
            sungeki_review::current_source_fingerprint(std::slice::from_ref(&asset), 1024).unwrap();
        Self {
            root,
            source,
            asset,
            expected,
        }
    }
    fn observe(&self, runtime: &Runtime) -> AssetFreshness {
        runtime.freshness.observe(
            runtime,
            "job",
            std::slice::from_ref(&self.asset),
            Some(&self.expected),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
async fn finished(runtime: &Runtime) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
fn expire(runtime: &Runtime) {
    runtime
        .freshness
        .entries
        .lock()
        .unwrap()
        .get_mut("job")
        .unwrap()
        .observed = Instant::now() - Duration::from_secs(6);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn asynchronous_observations_detect_change_missing_and_unavailable_without_editing_assets() {
    let f = Fixture::new();
    let runtime = Runtime::default();
    let first = f.observe(&runtime);
    assert_eq!(first.status, Status::Unchecked);
    assert!(first.checking);
    finished(&runtime).await;
    let matched = f.observe(&runtime);
    assert_eq!(matched.status, Status::Matching);
    assert!(matched.checked_at_ms.is_some());
    assert!(!matched.checking);
    assert_eq!(f.observe(&runtime).checked_at_ms, matched.checked_at_ms); // TTL avoids rereading per poll.
    fs::write(&f.source, b"changed").unwrap();
    expire(&runtime);
    assert!(f.observe(&runtime).checking);
    finished(&runtime).await;
    assert_eq!(f.observe(&runtime).status, Status::Changed);
    fs::remove_file(&f.source).unwrap();
    expire(&runtime);
    f.observe(&runtime);
    finished(&runtime).await;
    assert_eq!(f.observe(&runtime).status, Status::Changed);
    fs::create_dir(&f.source).unwrap();
    expire(&runtime);
    f.observe(&runtime);
    finished(&runtime).await;
    let unavailable = f.observe(&runtime);
    assert_eq!(unavailable.status, Status::Unavailable);
    assert!(unavailable.message.is_some());
    assert!(unavailable.last_known_changed);
    assert_eq!(
        f.asset.content_hash.as_deref(),
        Some(f.asset.probe.sha256())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn busy_scan_slot_returns_unchecked_without_queueing_or_blocking_and_retries_later() {
    let f = Fixture::new();
    let runtime = Runtime::default();
    let slot = runtime
        .freshness
        .slots
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    for _ in 0..100 {
        let result = f.observe(&runtime);
        assert_eq!(result.status, Status::Unchecked);
        assert!(!result.checking);
    }
    assert!(runtime.freshness.entries.lock().unwrap().is_empty());
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    drop(slot);
    assert!(f.observe(&runtime).checking);
    finished(&runtime).await;
    assert_eq!(f.observe(&runtime).status, Status::Matching);
}

#[test]
fn no_baseline_is_never_presented_as_matching() {
    let f = Fixture::new();
    let runtime = Runtime::default();
    assert_eq!(
        runtime
            .freshness
            .observe(&runtime, "job", std::slice::from_ref(&f.asset), None)
            .status,
        Status::Unchecked
    );
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_observations_are_bounded_and_evicted_jobs_are_rechecked() {
    let f = Fixture::new();
    let runtime = Runtime::default();
    for index in 0..=MAX_ENTRIES {
        let value = runtime.freshness.observe(
            &runtime,
            &format!("job-{index}"),
            std::slice::from_ref(&f.asset),
            Some(&f.expected),
        );
        assert!(value.checking);
        finished(&runtime).await;
    }
    {
        let entries = runtime.freshness.entries.lock().unwrap();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert!(!entries.contains_key("job-0"));
    }
    let restarted = runtime.freshness.observe(
        &runtime,
        "job-0",
        std::slice::from_ref(&f.asset),
        Some(&f.expected),
    );
    assert_eq!(restarted.status, Status::Unchecked);
    assert!(restarted.checking);
    finished(&runtime).await;
    assert_eq!(runtime.freshness.entries.lock().unwrap().len(), MAX_ENTRIES);
}
