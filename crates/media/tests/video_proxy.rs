#[path = "video_proxy/faults.rs"]
mod faults;
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{
    ProcessSpec, run_process,
    video_probe::{
        ProbeRuntime,
        frames::{
            PreparedVideo,
            proxy::{ProxyEncoding, ProxySettings},
        },
        probe,
    },
};
use sungeki_project::TimeUs;
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires real ffmpeg/ffprobe and SSD root"]
fn real_proxy_preserves_vfr_display_and_owned_cleanup() {
    for encoding in [ProxyEncoding::Png, ProxyEncoding::Qtrle] {
        verify_encoding(encoding);
    }
}
fn verify_encoding(encoding: ProxyEncoding) {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("video-proxy-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let flag = AtomicBool::new(false);
    let run = |args: &[&str]| {
        let args: Vec<_> = args.iter().map(Into::into).collect();
        let output = run_process(
            &ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(30),
                max_output_bytes: 4 * 1024 * 1024,
            },
            &flag,
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
        "testsrc2=size=320x240:rate=30:duration=1",
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
    let path = root.0.join("rotated.mp4");
    let bytes = fs::read(&path).unwrap();
    let runtime = ProbeRuntime {
        ffprobe: &ffprobe,
        root: &root.0,
        timeout: Duration::from_secs(30),
        max_source_bytes: 1024 * 1024,
    };
    let original_probe = probe(&path, &runtime, &flag).unwrap();
    let original = PreparedVideo::open(&path, &original_probe, &runtime, &flag).unwrap();
    let proxy = original
        .generate_proxy(
            ProxySettings {
                max_edge: 160,
                encoding,
            },
            &ffmpeg,
            &runtime,
            &flag,
        )
        .unwrap();
    assert_eq!(proxy.video.frame_count(), 5);
    assert!(proxy.video.probe().metadata.video.width < 320);
    assert!(proxy.video.probe().metadata.audio.is_empty());
    let raster = &proxy.video.probe().metadata.video;
    let filter = format!("scale={}:{}:flags=bilinear", raster.width, raster.height);
    let reference = run(&[
        "-nostdin",
        "-v",
        "error",
        "-noautorotate",
        "-i",
        "rotated.mp4",
        "-vf",
        &filter,
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    let frame_bytes = raster.width as usize * raster.height as usize * 4;
    for (time, index) in [
        (0, 0),
        (99999, 0),
        (100000, 1),
        (233334, 2),
        (500000, 3),
        (966667, 4),
    ] {
        assert_eq!(original.frame_index_at(TimeUs(time)).unwrap(), Some(index));
        let frame = proxy
            .video
            .decode_at(TimeUs(time), &ffmpeg, Duration::from_secs(10), &flag)
            .unwrap()
            .unwrap();
        assert_eq!(frame.frame_index, index);
        assert_eq!(
            frame.rgba,
            reference[index * frame_bytes..(index + 1) * frame_bytes]
        );
        assert_eq!(
            proxy
                .timeline
                .frame_at(TimeUs(time))
                .unwrap()
                .unwrap()
                .proxy_pts,
            frame.pts
        );
    }
    assert!(
        original
            .generate_proxy(
                ProxySettings {
                    max_edge: 160,
                    encoding
                },
                &ffmpeg,
                &runtime,
                &AtomicBool::new(true)
            )
            .is_err()
    );
    drop(proxy);
    faults::check(&original, &runtime, &root.0);
    drop(original);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
    run(&[
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:size=128x96:rate=10:duration=0.3,format=rgba,geq=r=0:g=0:b=255:a='if(eq(N,0),128,if(eq(N,1),0,255))'",
        "-c:v",
        "png",
        "-threads",
        "1",
        "-pix_fmt",
        "rgba",
        "alpha.mov",
    ]);
    let alpha_path = root.0.join("alpha.mov");
    let alpha_probe = probe(&alpha_path, &runtime, &flag).unwrap();
    let alpha = PreparedVideo::open(&alpha_path, &alpha_probe, &runtime, &flag).unwrap();
    let reduced = alpha
        .generate_proxy(
            ProxySettings {
                max_edge: 64,
                encoding,
            },
            &ffmpeg,
            &runtime,
            &flag,
        )
        .unwrap();
    let image = reduced
        .video
        .decode_at(TimeUs(0), &ffmpeg, Duration::from_secs(10), &flag)
        .unwrap()
        .unwrap();
    assert_eq!((image.stream.width, image.stream.height), (64, 48));
    assert_eq!(reduced.video.frame_count(), 3);
    let alpha_reference = run(&[
        "-nostdin",
        "-v",
        "error",
        "-i",
        "alpha.mov",
        "-vf",
        "scale=64:48:flags=bilinear",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    assert_eq!(image.rgba, alpha_reference[..64 * 48 * 4]);
    for index in 1..3 {
        let frame = reduced
            .video
            .decode_at(
                TimeUs(index as u64 * 100_000),
                &ffmpeg,
                Duration::from_secs(10),
                &flag,
            )
            .unwrap()
            .unwrap();
        let size = 64 * 48 * 4;
        assert_eq!(
            frame.rgba,
            alpha_reference[index * size..(index + 1) * size]
        );
        assert!(
            frame
                .rgba
                .chunks_exact(4)
                .all(|p| p[3] == if index == 1 { 0 } else { 255 })
        );
    }
    assert!(
        image
            .rgba
            .chunks_exact(4)
            .all(|p| p[0] == 0 && p[1] == 0 && p[2] >= 254 && (1..255).contains(&p[3]))
    );
    drop(reduced);
    drop(alpha);
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 3);
}
