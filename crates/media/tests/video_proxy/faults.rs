use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use sungeki_media::{
    StopReason,
    video_probe::{
        ProbeError, ProbeRuntime,
        frames::{PreparedVideo, proxy::ProxySettings},
    },
};

pub fn check(original: &PreparedVideo, runtime: &ProbeRuntime<'_>, root: &Path) {
    let staging: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("video-probe-")
        })
        .collect();
    assert_eq!(staging.len(), 1);
    let before = fs::read(staging[0].join("source.media")).unwrap();
    for mode in ["cancel", "deadline", "failure", "invalid-output"] {
        let marker = root.join(format!("{mode}.ready"));
        let encoder = root.join(format!("{mode}.sh"));
        let tail = match mode {
            "failure" => "exit 7",
            "invalid-output" => "exit 0",
            _ => "exec /bin/sleep 30",
        };
        let quoted = marker.to_str().unwrap().replace('\'', "'\\''");
        fs::write(&encoder, format!("#!/bin/sh\nfor last; do :; done\nprintf partial > \"$last\"\nprintf '%s' \"$$\" > '{quoted}'\n{tail}\n")).unwrap();
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o700)).unwrap();
        let flag = AtomicBool::new(false);
        let runtime = ProbeRuntime {
            ffprobe: runtime.ffprobe,
            root: runtime.root,
            timeout: if mode == "deadline" {
                // Leave startup headroom on a loaded SSD; the fake encoder sleeps 30s.
                // The PID marker below still proves that deadline cleanup killed a started worker.
                Duration::from_secs(2)
            } else {
                Duration::from_secs(5)
            },
            max_source_bytes: runtime.max_source_bytes,
        };
        let started = Instant::now();
        let result = std::thread::scope(|scope| {
            if mode == "cancel" {
                scope.spawn(|| {
                    let deadline = Instant::now() + Duration::from_secs(3);
                    while !marker.exists() && Instant::now() < deadline {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    flag.store(true, Ordering::Release);
                    assert!(marker.exists(), "encoder never started");
                });
            }
            original.generate_proxy(ProxySettings::new(160), &encoder, &runtime, &flag)
        });
        match mode {
            "cancel" => assert!(matches!(
                result,
                Err(ProbeError::Stopped(StopReason::Cancelled))
            )),
            "deadline" => assert!(matches!(
                result,
                Err(ProbeError::Stopped(StopReason::Deadline))
            )),
            _ => assert!(matches!(result, Err(ProbeError::Worker(_)))),
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        let pid: i32 = fs::read_to_string(&marker).unwrap().parse().unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "owned worker still exists"
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        assert_eq!(
            fs::read_dir(&staging[0]).unwrap().count(),
            1,
            "partial output survived"
        );
        assert_eq!(fs::read(staging[0].join("source.media")).unwrap(), before);
        assert_eq!(
            fs::read_dir(root)
                .unwrap()
                .filter(|e| e
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("video-probe-"))
                .count(),
            1
        );
        fs::remove_file(marker).unwrap();
        fs::remove_file(encoder).unwrap();
    }
}
