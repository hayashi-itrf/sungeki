use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{
    ProcessSpec, StopReason, run_process,
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

#[test]
#[ignore = "requires real ffmpeg and an SSD test root"]
fn exact_completed_movie_frames_reject_changed_sources_and_clean_snapshots() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("rendered-frame-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let run = |args: &[&str]| {
        let args = args.iter().map(Into::into).collect::<Vec<_>>();
        let output = run_process(
            &ProcessSpec {
                program: &ffmpeg,
                args: &args,
                directory: &root.0,
                timeout: Duration::from_secs(20),
                max_output_bytes: 1024 * 1024,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(output.succeeded(), "{output:?}");
        output.stdout
    };
    run(&[
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=160x120:rate=3:duration=1",
        "-c:v",
        "libx264",
        "-an",
        "source.mp4",
    ]);
    let reference = run(&[
        "-v",
        "error",
        "-i",
        "source.mp4",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "pipe:1",
    ]);
    let frame_bytes = 160 * 120 * 3;
    assert_eq!(reference.len(), 3 * frame_bytes);
    assert_ne!(
        reference[..frame_bytes],
        reference[frame_bytes..2 * frame_bytes]
    );
    let source = root.0.join("source.mp4");
    let original = fs::read(&source).unwrap();
    let hash = format!("{:x}", Sha256::digest(&original));
    let runtime = ProbeRuntime {
        ffprobe: &ffmpeg,
        root: &root.0,
        timeout: Duration::from_secs(20),
        max_source_bytes: 1024 * 1024,
    };
    let flag = AtomicBool::new(false);
    let mut request = FrameRequest {
        source: &source,
        sha256: &hash,
        byte_length: original.len() as u64,
        frame_index: 0,
        frame_count: 3,
        width: 160,
        height: 120,
    };
    for index in [2, 0, 1, 2] {
        request.frame_index = index;
        let bytes = png(&request, &ffmpeg, &runtime, &flag).unwrap();
        fs::write(root.0.join("frame.png"), bytes).unwrap();
        let actual = run(&[
            "-v",
            "error",
            "-i",
            "frame.png",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ]);
        let start = index as usize * frame_bytes;
        assert_eq!(actual, reference[start..start + frame_bytes]);
        assert!(
            !fs::read_dir(&root.0)
                .unwrap()
                .any(|e| e.unwrap().file_type().unwrap().is_dir())
        );
    }
    assert_eq!(fs::read(&source).unwrap(), original);
    request.frame_index = 3;
    assert!(matches!(
        png(&request, &ffmpeg, &runtime, &flag),
        Err(ProbeError::Invalid(_))
    ));
    request.frame_index = 0;
    assert!(matches!(
        png(&request, &ffmpeg, &runtime, &AtomicBool::new(true)),
        Err(ProbeError::Stopped(StopReason::Cancelled))
    ));
    let mut changed = original.clone();
    changed[0] ^= 1;
    fs::write(&source, changed).unwrap();
    assert!(matches!(
        png(&request, &ffmpeg, &runtime, &flag),
        Err(ProbeError::Changed)
    ));
    fs::write(&source, &original).unwrap();
    assert!(
        png(
            &request,
            std::path::Path::new("/usr/bin/false"),
            &runtime,
            &flag
        )
        .is_err()
    );
    assert!(
        !fs::read_dir(&root.0)
            .unwrap()
            .any(|e| e.unwrap().file_type().unwrap().is_dir())
    );
}
