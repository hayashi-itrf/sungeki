use std::{
    fs, io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use sungeki_media::{
    StopReason,
    video_probe::{ProbeError, frames::PreparedVideo},
};
pub fn check(video: &PreparedVideo, ffmpeg: &Path, root: &Path, reference: &[u8]) {
    let flag = AtomicBool::new(false);
    let mut count = 0;
    video
        .decode_range(1..4, ffmpeg, Duration::from_secs(10), &flag, |frame| {
            assert_eq!(frame.frame_index, count + 1);
            let size = 160 * 120 * 4;
            assert!(frame.rgba == reference[(count + 1) * size..(count + 2) * size]);
            count += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(count, 3);
    let result = video.decode_range(0..5, ffmpeg, Duration::from_secs(10), &flag, |_| {
        flag.store(true, Ordering::Release);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(ProbeError::Stopped(StopReason::Cancelled))
    ));
    flag.store(false, Ordering::Release);
    let result = video.decode_range(0..5, ffmpeg, Duration::from_secs(10), &flag, |_| {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "render consumer failed",
        ))
    });
    assert!(matches!(result, Err(ProbeError::Io(_))));
    for range in [0..0, 4..6] {
        assert!(
            video
                .decode_range(range, ffmpeg, Duration::from_secs(10), &flag, |_| panic!())
                .is_err()
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let worker = root.join("sequence-worker");
        for (script, kind) in [
            ("printf partial", "short"),
            ("sleep 5", "timeout"),
            ("echo broken >&2; exit 0", "stderr"),
        ] {
            fs::write(&worker, format!("#!/bin/sh\n{script}\n")).unwrap();
            fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
            let timeout = if kind == "timeout" {
                Duration::from_millis(100)
            } else {
                Duration::from_secs(3)
            };
            let result = video.decode_range(0..1, &worker, timeout, &flag, |_| panic!());
            assert!(match (kind, result) {
                ("short", Err(ProbeError::Invalid(_)))
                | ("stderr", Err(ProbeError::Worker(_)))
                | ("timeout", Err(ProbeError::Stopped(StopReason::Deadline))) => true,
                (_, other) => panic!("{kind}: {other:?}"),
            });
        }
    }
}
