//! Bridge parent-job cancellation into the atomic flag used by media workers.
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use sungeki_media::{StopReason, video_probe::ProbeError};
use sungeki_protocol::ApiError;
struct CancelOnDrop<'a>(&'a AtomicBool);
impl Drop for CancelOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
pub(crate) fn run<T: Send>(
    shutdown: &AtomicBool,
    mut cancelled: impl FnMut() -> bool,
    work: impl FnOnce(&AtomicBool) -> Result<T, ApiError> + Send,
) -> Result<T, ApiError> {
    let stopped = || super::error(ProbeError::Stopped(StopReason::Cancelled));
    if shutdown.load(Ordering::SeqCst) || cancelled() {
        return Err(stopped());
    }
    let flag = AtomicBool::new(false);
    std::thread::scope(|scope| {
        // Set the stop flag before scope joins if the caller's callback panics.
        let _cleanup = CancelOnDrop(&flag);
        let worker = scope.spawn(|| work(&flag));
        while !worker.is_finished() {
            if shutdown.load(Ordering::SeqCst) || cancelled() {
                flag.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let result = worker
            .join()
            .map_err(|_| ApiError::io("動画検査の実行処理が停止しました"))?;
        if flag.load(Ordering::SeqCst) || shutdown.load(Ordering::SeqCst) || cancelled() {
            Err(stopped())
        } else {
            result
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, time::Instant};
    #[test]
    fn parent_cancel_settles_owned_worker_without_stopping_other_jobs() {
        let root =
            std::env::temp_dir().join(format!("video-probe-cancel-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.mov");
        fs::write(&source, [42u8; 4096]).unwrap();
        let script = root.join("probe.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf x > '{}'\nsleep 30\n",
                root.join("ready").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let shutdown = AtomicBool::new(false);
        let started = Instant::now();
        let result = run(
            &shutdown,
            || root.join("ready").exists(),
            |flag| {
                sungeki_media::video_probe::probe(
                    &source,
                    &sungeki_media::video_probe::ProbeRuntime {
                        ffprobe: &script,
                        root: &root,
                        timeout: Duration::from_secs(40),
                        max_source_bytes: 4096,
                    },
                    flag,
                )
                .map_err(super::super::error)
            },
        );
        assert_eq!(
            result.unwrap_err().code,
            sungeki_protocol::ErrorCode::Cancelled
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!shutdown.load(Ordering::SeqCst));
        let mut files: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, ["probe.sh", "ready", "source.mov"]);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pre_cancel_does_not_start_work_and_success_keeps_result() {
        let shutdown = AtomicBool::new(false);
        let result: Result<(), _> = run(&shutdown, || true, |_| panic!("must not start"));
        assert!(result.is_err());
        assert_eq!(run(&shutdown, || false, |_| Ok(42)).unwrap(), 42);
        shutdown.store(true, Ordering::SeqCst);
        let result: Result<(), _> = run(&shutdown, || false, |_| panic!("must not start"));
        assert!(result.is_err());
    }
}
