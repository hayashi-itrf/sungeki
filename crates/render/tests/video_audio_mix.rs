#![cfg(unix)]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::video_probe::{ProbeRuntime, probe};
use sungeki_project::{AssetLocator, AssetProbe, ClipContent, Project, TimeUs};
use sungeki_render::{RenderPlan, RenderRuntime, audio_preview_plan, audio_preview_staged};
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn command(program: &Path, args: &[&str], output: &Path) {
    let result = Command::new(program)
        .args(args)
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
#[ignore = "requires real FFmpeg/ffprobe and SSD root"]
fn hidden_video_two_audio_streams_share_original_but_mix_independently() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let ffprobe = PathBuf::from(std::env::var_os("VVM_TEST_FFPROBE").unwrap());
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("video-mix-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let source = root.0.join("source.mov");
    command(
        &ffmpeg,
        &[
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=32x24:rate=5:duration=1",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.4|0.2:s=48000:d=1",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=-0.1|-0.3:s=48000:d=1",
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-map",
            "2:a",
            "-c:v",
            "libx264",
            "-bf",
            "0",
            "-c:a",
            "pcm_f32le",
        ],
        &source,
    );
    let original = fs::read(&source).unwrap();
    let flag = AtomicBool::new(false);
    let probe = probe(
        &source,
        &ProbeRuntime {
            ffprobe: &ffprobe,
            root: &root.0,
            timeout: Duration::from_secs(30),
            max_source_bytes: 1048576,
        },
        &flag,
    )
    .unwrap();
    assert_eq!(probe.metadata.audio.len(), 2);
    let mut p: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    p.assets[0].probe = AssetProbe::Video(probe.clone());
    p.assets[0].content_hash = Some(probe.sha256.clone());
    p.assets[0].locator = AssetLocator::Absolute {
        path: source.to_str().unwrap().into(),
    };
    p.canvas.fps.numerator = 10;
    p.canvas.fps.denominator = 1;
    p.tracks[0].visible = false;
    p.clips[0].start_us = TimeUs(100000);
    p.clips[0].duration_us = TimeUs(800000);
    let ClipContent::Video { playback, .. } = &mut p.clips[0].content else {
        panic!()
    };
    playback.source_start_us = TimeUs(250000);
    playback.source_duration_us = TimeUs(200000);
    playback.looped = true;
    playback.audio_stream = Some(probe.metadata.audio[0].index);
    playback.gain_db = -6.020599913279624;
    let mut second = p.clips[0].clone();
    second.id = uuid::Uuid::new_v4().to_string();
    let ClipContent::Video { playback, .. } = &mut second.content else {
        panic!()
    };
    playback.audio_stream = Some(probe.metadata.audio[1].index);
    playback.gain_db = 0.0;
    p.clips.push(second);
    let plan = audio_preview_plan(RenderPlan::new(&p, |_| Ok(())).unwrap());
    let runtime = RenderRuntime {
        vrm: None,
        python: PathBuf::from("/unused"),
        image_worker: PathBuf::from("/unused"),
        font: PathBuf::from("/unused"),
        font_sha256: "0".repeat(64),
        ffmpeg: ffmpeg.clone(),
        ffprobe,
        deadline: Duration::from_secs(30),
    };
    let job = root.0.join("job");
    fs::create_dir(&job).unwrap();
    let artifact = audio_preview_staged(&plan, &runtime.audio(), &job, &flag).unwrap();
    assert!(artifact.byte_length > 0);
    let raw = root.0.join("mix.f32");
    command(
        &ffmpeg,
        &[
            "-nostdin",
            "-v",
            "error",
            "-i",
            job.join("mix.wav").to_str().unwrap(),
            "-f",
            "f32le",
            "-c:a",
            "pcm_f32le",
        ],
        &raw,
    );
    let bytes = fs::read(raw).unwrap();
    assert_eq!(bytes.len(), 43200 * 8);
    for (i, frame) in bytes.chunks_exact(8).enumerate() {
        let left = f32::from_le_bytes(frame[..4].try_into().unwrap());
        let right = f32::from_le_bytes(frame[4..].try_into().unwrap());
        let expected = if i < 4800 { (0.0, 0.0) } else { (0.1, -0.2) };
        assert!((left - expected.0).abs() < 1e-6, "left {i}: {left}");
        assert!((right - expected.1).abs() < 1e-6, "right {i}: {right}");
    }
    let inspection = root.0.join("inspection");
    fs::create_dir(&inspection).unwrap();
    sungeki_render::inspect_output_audio(&plan, &runtime, &inspection, &flag).unwrap();
    assert_eq!(
        fs::read(job.join("mix.wav")).unwrap(),
        fs::read(inspection.join("mix.wav")).unwrap()
    );
    p.tracks[0].muted = true;
    let muted = audio_preview_plan(RenderPlan::new(&p, |_| Ok(())).unwrap());
    // The silent plan must not read the unused video original.
    fs::rename(&source, root.0.join("offline.mov")).unwrap();
    let silent = root.0.join("silent");
    fs::create_dir(&silent).unwrap();
    audio_preview_staged(&muted, &runtime.audio(), &silent, &flag).unwrap();
    assert!(
        !fs::read_dir(&silent)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("video-audio-"))
    );
    assert_eq!(fs::read(root.0.join("offline.mov")).unwrap(), original);
    let decoded = fs::read_dir(&job)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("video-audio-")
        })
        .count();
    assert_eq!(decoded, 2);
}
