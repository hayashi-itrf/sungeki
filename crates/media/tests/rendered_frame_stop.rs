//! Failure lifecycle coverage with an owned fake decoder, not video-quality QA.
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use sungeki_media::{
    StopReason,
    video_probe::{
        ProbeError, ProbeRuntime,
        rendered_frame::{FrameRequest, png},
    },
};

struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn verify(cancel_during_decode: bool) {
    let parent = std::env::var_os("VVM_MEDIA_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let root = Root(parent.join(format!("render-frame-stop-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&root.0).unwrap();
    let source = root.0.join("source.mp4");
    let bytes = b"owned decoder lifecycle fixture";
    fs::write(&source, bytes).unwrap();
    let hash = format!("{:x}", Sha256::digest(bytes));
    let worker = root.0.join("decoder.sh");
    fs::write(
        &worker,
        "#!/bin/sh\necho $$ > ../worker.pid\nexec /bin/sleep 30\n",
    )
    .unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let marker = root.0.join("worker.pid");
    let flag = AtomicBool::new(false);
    let request = FrameRequest {
        source: &source,
        sha256: &hash,
        byte_length: bytes.len() as u64,
        frame_index: 0,
        frame_count: 1,
        width: 16,
        height: 16,
    };
    let runtime = ProbeRuntime {
        ffprobe: &worker,
        root: &root.0,
        timeout: Duration::from_secs(if cancel_during_decode { 5 } else { 1 }),
        max_source_bytes: 1024,
    };
    let started = Instant::now();
    let result = thread::scope(|scope| {
        let pending = scope.spawn(|| png(&request, &worker, &runtime, &flag));
        if cancel_during_decode {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !marker.exists() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            // Signal even on startup failure, so the test never strands its worker.
            flag.store(true, Ordering::Release);
        }
        pending.join().unwrap()
    });
    let expected = if cancel_during_decode {
        StopReason::Cancelled
    } else {
        StopReason::Deadline
    };
    assert!(
        matches!(result, Err(ProbeError::Stopped(reason)) if reason == expected),
        "{result:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(4));
    let pid: i32 = fs::read_to_string(marker)
        .expect("decoder actually started")
        .trim()
        .parse()
        .unwrap();
    // Signal 0 checks existence only; never signal a potentially reused process.
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert!(
        !fs::read_dir(&root.0)
            .unwrap()
            .any(|entry| entry.unwrap().file_type().unwrap().is_dir())
    );
}

#[test]
fn cancellation_after_decoder_start_reaps_worker_and_removes_snapshot() {
    verify(true);
}

#[test]
fn deadline_after_decoder_start_reaps_worker_and_removes_snapshot() {
    verify(false);
}
