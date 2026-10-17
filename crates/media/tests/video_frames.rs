#[path = "video_frames/sequence_checks.rs"]
mod sequence_checks;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use sungeki_media::{
    StopReason,
    video_probe::{ProbeError, ProbeRuntime, frames::PreparedVideo, probe},
};
use sungeki_project::TimeUs;
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
#[ignore = "requires real ffmpeg/ffprobe and an SSD test root"]
fn real_vfr_pixels_rotation_snapshot_and_cleanup() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("video-frames-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let run = |args: &[&str]| {
        let args: Vec<_> = args.iter().map(Into::into).collect();
        let output = sungeki_media::run_process(
            &sungeki_media::ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(30),
                max_output_bytes: 4 * 1024 * 1024,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(output.succeeded(), "{output:?}");
        output.stdout
    };
    run(&[
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=30:duration=1",
        "-vf",
        "setsar=4/3,select='eq(n,0)+eq(n,3)+eq(n,7)+eq(n,15)+eq(n,29)'",
        "-fps_mode",
        "vfr",
        "-c:v",
        "libx264",
        "-bf",
        "0",
        "-an",
        "source.mp4",
    ]);
    run(&[
        "-nostdin",
        "-v",
        "error",
        "-display_rotation",
        "90",
        "-i",
        "source.mp4",
        "-c",
        "copy",
        "rotated.mp4",
    ]);
    let reference = run(&[
        "-nostdin",
        "-v",
        "error",
        "-noautorotate",
        "-i",
        "rotated.mp4",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    let source = root.0.join("rotated.mp4");
    let original = fs::read(&source).unwrap();
    let flag = AtomicBool::new(false);
    let runtime = ProbeRuntime {
        ffprobe: &ffprobe,
        root: &root.0,
        timeout: Duration::from_secs(30),
        max_source_bytes: 1024 * 1024,
    };
    let saved = probe(&source, &runtime, &flag).unwrap();
    let prepared = PreparedVideo::open(&source, &saved, &runtime, &flag).unwrap();
    // Independent FFmpeg transpose after RGBA conversion avoids chroma-resampling differences.
    let auto = run(&[
        "-nostdin",
        "-v",
        "error",
        "-noautorotate",
        "-i",
        "rotated.mp4",
        "-frames:v",
        "1",
        "-vf",
        "format=rgba,transpose=cclock,setsar=1",
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    let mut square = prepared
        .decode_at(TimeUs(0), &ffmpeg, Duration::from_secs(30), &flag)
        .unwrap()
        .unwrap();
    square.stream.sample_aspect_ratio = None;
    let square = square.into_display().unwrap();
    assert_eq!((square.width, square.height), (120, 160));
    assert!(square.rgba == auto, "RGBA transpose differs");
    let display = prepared
        .decode_at(TimeUs(0), &ffmpeg, Duration::from_secs(30), &flag)
        .unwrap()
        .unwrap()
        .into_display()
        .unwrap();
    assert_eq!((display.width, display.height), (120, 214));
    assert_eq!(prepared.frame_count(), 5);
    assert_eq!(prepared.probe(), &saved);
    assert_eq!(saved.metadata.video.rotation_degrees, Some(90.0));
    assert_eq!(
        saved.metadata.video.sample_aspect_ratio.unwrap().numerator,
        4
    );
    assert_eq!(reference.len(), 160 * 120 * 4 * 5);
    sequence_checks::check(&prepared, &ffmpeg, &root.0, &reference);
    for (time, index) in [
        (0, 0),
        (99_999, 0),
        (100_000, 1),
        (233_333, 1),
        (233_334, 2),
        (499_999, 2),
        (500_000, 3),
        (966_666, 3),
        (966_667, 4),
    ] {
        let frame = prepared
            .decode_at(TimeUs(time), &ffmpeg, Duration::from_secs(30), &flag)
            .unwrap_or_else(|e| panic!("time {time}: {e:?}"))
            .unwrap();
        assert_eq!(frame.frame_index, index, "time {time}");
        assert_eq!(frame.stream, saved.metadata.video);
        let size = 160 * 120 * 4;
        assert_eq!(frame.rgba, reference[index * size..(index + 1) * size]);
    }
    assert!(
        prepared
            .decode_at(TimeUs(1_000_000), &ffmpeg, Duration::from_secs(30), &flag)
            .is_err()
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    flag.store(true, Ordering::Release);
    assert!(matches!(
        prepared.decode_at(TimeUs(0), &ffmpeg, Duration::from_secs(30), &flag),
        Err(ProbeError::Stopped(StopReason::Cancelled))
    ));
    flag.store(false, Ordering::Release);
    // Decode order differs from presentation order with B frames.
    run(&[
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=30:duration=1",
        "-c:v",
        "libx264",
        "-bf",
        "2",
        "-an",
        "bframes.mp4",
    ]);
    let bpath = root.0.join("bframes.mp4");
    let bp = probe(&bpath, &runtime, &flag).unwrap();
    let bv = PreparedVideo::open(&bpath, &bp, &runtime, &flag).unwrap();
    let all = run(&[
        "-nostdin",
        "-v",
        "error",
        "-i",
        "bframes.mp4",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    assert_eq!(all.len(), 30 * 160 * 120 * 4);
    for (time, index) in [(0, 0), (33334, 1), (500000, 15), (966667, 29)] {
        let frame = bv
            .decode_at(TimeUs(time), &ffmpeg, Duration::from_secs(30), &flag)
            .unwrap()
            .unwrap();
        assert_eq!(frame.frame_index, index);
        assert_eq!(
            frame.rgba,
            all[index * 160 * 120 * 4..(index + 1) * 160 * 120 * 4]
        );
    }
    drop(bv);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let worker = root.0.join("error-worker");
        fs::write(&worker, "#!/bin/sh\necho decode-error >&2\nexit 0\n").unwrap();
        fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(
            prepared.decode_at(TimeUs(0), &worker, Duration::from_secs(3), &flag),
            Err(ProbeError::Worker(_))
        ));
    }
    fs::write(&source, b"replaced original").unwrap();
    assert!(matches!(
        PreparedVideo::open(&source, &saved, &runtime, &flag),
        Err(ProbeError::Changed)
    ));
    let frame = prepared
        .decode_at(TimeUs(500_000), &ffmpeg, Duration::from_secs(30), &flag)
        .unwrap()
        .unwrap();
    assert_eq!(frame.rgba, reference[3 * 160 * 120 * 4..4 * 160 * 120 * 4]);
    drop(prepared);
    assert!(!fs::read_dir(&root.0).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("video-probe-")
    }));
}
