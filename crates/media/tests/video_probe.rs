#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use sungeki_media::{
    StopReason,
    video_probe::{ProbeError, ProbeRuntime, Rational, probe},
};
struct Root(PathBuf);
impl Root {
    fn new(base: &Path) -> Self {
        let path = base.join(format!("video-inspect-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn worker(&self, body: &str) -> PathBuf {
        let p = self.0.join("worker");
        fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        p
    }
    fn clean(&self) {
        assert!(!fs::read_dir(&self.0).unwrap().any(|p| {
            p.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("video-probe-")
        }));
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn failure_cancel_limits_and_links_leave_no_snapshot() {
    let root = Root::new(&std::env::temp_dir());
    let source = root.0.join("original.mp4");
    fs::write(&source, b"original unchanged").unwrap();
    let flag = AtomicBool::new(false);
    for (script, expected) in [
        ("echo broken; exit 2", "worker"),
        ("echo '{}'", "invalid"),
        ("head -c 1100000 /dev/zero", "output"),
        ("sleep 5", "deadline"),
    ] {
        let worker = root.worker(script);
        let result = probe(
            &source,
            &ProbeRuntime {
                ffprobe: &worker,
                root: &root.0,
                timeout: if expected == "deadline" {
                    Duration::from_millis(100)
                } else {
                    Duration::from_secs(3)
                },
                max_source_bytes: 1000,
            },
            &flag,
        );
        assert!(match (expected, result) {
            ("worker", Err(ProbeError::Worker(_)))
            | ("invalid", Err(ProbeError::Invalid(_)))
            | ("output", Err(ProbeError::Stopped(StopReason::OutputLimit)))
            | ("deadline", Err(ProbeError::Stopped(StopReason::Deadline))) => true,
            (_, other) => panic!("{expected}: {other:?}"),
        });
        root.clean();
    }
    let worker = root.worker("sleep 5");
    let runtime = ProbeRuntime {
        ffprobe: &worker,
        root: &root.0,
        timeout: Duration::from_secs(3),
        max_source_bytes: 1000,
    };
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(50));
            flag.store(true, Ordering::Release);
        });
        assert!(matches!(
            probe(&source, &runtime, &flag),
            Err(ProbeError::Stopped(StopReason::Cancelled))
        ));
    });
    root.clean();
    assert!(matches!(
        probe(&source, &runtime, &flag),
        Err(ProbeError::Stopped(StopReason::Cancelled))
    ));
    flag.store(false, Ordering::Release);
    assert!(matches!(
        probe(
            &source,
            &ProbeRuntime {
                max_source_bytes: 1,
                ..runtime
            },
            &flag
        ),
        Err(ProbeError::Invalid(_))
    ));
    let link = root.0.join("link.mp4");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    assert!(probe(&link, &runtime, &flag).is_err());
    assert!(probe(&root.0, &runtime, &flag).is_err());
    root.clean();
    assert_eq!(fs::read(source).unwrap(), b"original unchanged");
}
#[test]
#[ignore = "requires real ffmpeg/ffprobe and an SSD test root"]
fn real_rotation_audio_and_variable_rate_metadata() {
    use sha2::{Digest, Sha256};
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root::new(&PathBuf::from(
        std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap(),
    ));
    let source = root.0.join("source.mp4");
    let run = |args: Vec<std::ffi::OsString>| {
        let result = sungeki_media::run_process(
            &sungeki_media::ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(30),
                max_output_bytes: 65536,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(result.succeeded(), "{:?}", result);
    };
    let args: Vec<_> = [
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=30:duration=1",
        "-itsoffset",
        "0.25",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=1",
        "-vf",
        "setsar=4/3",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-shortest",
    ]
    .into_iter()
    .map(Into::into)
    .chain([source.as_os_str().to_owned()])
    .collect();
    run(args);
    let rotated = root.0.join("rotated.mp4");
    run(["-nostdin", "-v", "error", "-display_rotation", "90", "-i"]
        .into_iter()
        .map(Into::into)
        .chain([source.as_os_str().to_owned()])
        .chain(["-c", "copy"].into_iter().map(Into::into))
        .chain([rotated.as_os_str().to_owned()])
        .collect());
    let runtime = ProbeRuntime {
        ffprobe: &ffprobe,
        root: &root.0,
        timeout: Duration::from_secs(30),
        max_source_bytes: 1024 * 1024,
    };
    let original = fs::read(&rotated).unwrap();
    let p = probe(&rotated, &runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(p.sha256, format!("{:x}", Sha256::digest(&original)));
    assert_eq!(p.byte_length, original.len() as u64);
    assert_eq!(
        (p.metadata.video.width, p.metadata.video.height),
        (160, 120)
    );
    assert_eq!(
        p.metadata.video.sample_aspect_ratio,
        Some(Rational {
            numerator: 4,
            denominator: 3
        })
    );
    assert_eq!(p.metadata.video.rotation_degrees, Some(90.0));
    assert!(p.metadata.video.display_matrix.is_some());
    assert_eq!(p.metadata.video.frame_count, Some(30));
    assert_eq!(p.metadata.audio.len(), 1);
    assert_eq!(p.metadata.audio[0].sample_rate, 48000);
    assert!(p.metadata.audio[0].clock.start_pts.unwrap() > 0);
    assert_eq!(fs::read(&rotated).unwrap(), original);
    root.clean();
    let mkv = root.0.join("source.mkv");
    run(["-nostdin", "-v", "error", "-i"]
        .into_iter()
        .map(Into::into)
        .chain([source.as_os_str().to_owned()])
        .chain(["-c", "copy"].into_iter().map(Into::into))
        .chain([mkv.as_os_str().to_owned()])
        .collect());
    let matroska = probe(&mkv, &runtime, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        matroska.metadata.container,
        sungeki_project::VideoContainer::Matroska
    );
    assert_eq!(p.metadata.container, sungeki_project::VideoContainer::Mov);
    root.clean();
    let vfr = root.0.join("variable.mp4");
    run([
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=30:duration=1",
        "-vf",
        "select='lt(n,10)+not(mod(n,3))'",
        "-fps_mode",
        "vfr",
        "-c:v",
        "libx264",
        "-an",
    ]
    .into_iter()
    .map(Into::into)
    .chain([vfr.as_os_str().to_owned()])
    .collect());
    let p = probe(&vfr, &runtime, &AtomicBool::new(false)).unwrap();
    assert!(p.metadata.audio.is_empty());
    assert_eq!(p.metadata.video.frame_count, Some(16));
    assert_ne!(
        p.metadata.video.average_frame_rate,
        p.metadata.video.nominal_frame_rate
    );
    root.clean();
    let bad = root.0.join("broken.mp4");
    fs::write(&bad, b"not a movie").unwrap();
    assert!(probe(&bad, &runtime, &AtomicBool::new(false)).is_err());
    root.clean();
    let playlist = root.0.join("list.txt");
    fs::write(&playlist, "ffconcat version 1.0\nfile source.mp4\n").unwrap();
    assert!(probe(&playlist, &runtime, &AtomicBool::new(false)).is_err());
    root.clean();
}
