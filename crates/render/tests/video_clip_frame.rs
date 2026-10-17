#![cfg(unix)]
#[path = "video_resolver/formal.rs"]
mod formal;
#[path = "video_resolver/checks.rs"]
mod resolver_checks;
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::video_probe::{ProbeRuntime, frames::PreparedVideo, probe};
use sungeki_project::{ClipContent, Project, TimeUs};
use sungeki_render::{stage_video_clip_frame, stage_video_frame};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
#[ignore = "requires real FFmpeg/ffprobe and configured SSD root"]
fn timeline_trim_and_loop_stage_the_correct_original_pixels() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("clip-frame-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let input = root.0.join("source.mp4");
    let flag = AtomicBool::new(false);
    let args: Vec<_> = [
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=32x24:rate=5:duration=1",
        "-c:v",
        "libx264",
        "-bf",
        "0",
        "-an",
    ]
    .into_iter()
    .map(Into::into)
    .chain([input.as_os_str().to_owned()])
    .collect();
    let output = sungeki_media::run_process(
        &sungeki_media::ProcessSpec {
            program: &ffmpeg,
            args: &args,
            directory: &root.0,
            timeout: Duration::from_secs(30),
            max_output_bytes: 65536,
        },
        &flag,
    )
    .unwrap();
    assert!(output.succeeded(), "{output:?}");
    let original = fs::read(&input).unwrap();
    let runtime = ProbeRuntime {
        ffprobe: &ffprobe,
        root: &root.0,
        timeout: Duration::from_secs(30),
        max_source_bytes: 1048576,
    };
    let probe = probe(&input, &runtime, &flag).unwrap();
    let video = PreparedVideo::open(&input, &probe, &runtime, &flag).unwrap();
    let project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    formal::check(&project, &probe, &input, &root.0, &ffmpeg, &ffprobe);
    let mut clip = project.clips[0].clone();
    clip.start_us = TimeUs(1_000_000);
    clip.duration_us = TimeUs(1_000_000);
    let ClipContent::Video { playback, .. } = &mut clip.content else {
        panic!()
    };
    playback.source_start_us = TimeUs(200_000);
    playback.source_duration_us = TimeUs(400_000);
    playback.looped = true;
    let mut hashes = Vec::new();
    for (i, (timeline, source)) in [
        (1_000_000, 200_000),
        (1_399_999, 599_999),
        (1_400_000, 200_000),
        (1_800_000, 200_000),
    ]
    .into_iter()
    .enumerate()
    {
        let staged = stage_video_clip_frame(
            &video,
            &clip,
            TimeUs(timeline),
            &root.0.join(format!("actual-{i}.rgba")),
            &ffmpeg,
            Duration::from_secs(30),
            &flag,
        )
        .unwrap()
        .unwrap();
        let expected = video
            .decode_at(TimeUs(source), &ffmpeg, Duration::from_secs(30), &flag)
            .unwrap()
            .unwrap();
        let expected =
            stage_video_frame(expected, &root.0.join(format!("expected-{i}.rgba")), &flag).unwrap();
        assert_eq!(staged.sha256, expected.sha256);
        hashes.push(staged.sha256);
    }
    assert_eq!(hashes[0], hashes[2]);
    assert_eq!(hashes[0], hashes[3]);
    assert_ne!(hashes[0], hashes[1]);
    for t in [999_999, 2_000_000] {
        let target = root.0.join(format!("outside-{t}.rgba"));
        assert!(
            stage_video_clip_frame(
                &video,
                &clip,
                TimeUs(t),
                &target,
                &ffmpeg,
                Duration::from_secs(30),
                &flag
            )
            .unwrap()
            .is_none()
        );
        assert!(!target.exists());
    }
    assert_eq!(fs::read(input).unwrap(), original);
    resolver_checks::check(video, &project, &root.0, &ffmpeg);
    assert!(!fs::read_dir(&root.0).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("video-probe-")
    }));
}
