//! Opt-in decoder benchmark, not a completed-video production test.
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use sungeki_media::video_probe::{ProbeRuntime, frames::PreparedVideo, probe};
fn measure(
    video: &PreparedVideo,
    ffmpeg: &Path,
    batch: usize,
    flag: &AtomicBool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut hash = Sha256::new();
    let mut frames = 0usize;
    let mut bytes = 0u64;
    let mut calls = 0usize;
    for first in (0..video.frame_count()).step_by(batch) {
        video.decode_range(
            first..(first + batch).min(video.frame_count()),
            ffmpeg,
            Duration::from_secs(300),
            flag,
            |frame| {
                assert_eq!(frame.frame_index, frames);
                hash.update(&frame.rgba);
                frames += 1;
                bytes += frame.rgba.len() as u64;
                Ok(())
            },
        )?;
        calls += 1;
    }
    Ok(
        serde_json::json!({"batch_frames":batch,"worker_calls":calls,"frames":frames,
        "rgba_bytes":bytes,"rgba_sha256":format!("{:x}",hash.finalize()),
        "seconds":started.elapsed().as_secs_f64()}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: video_decode_benchmark <source> <ffmpeg> <ffprobe> <private-root>".into(),
        );
    }
    let flag = AtomicBool::new(false);
    let runtime = ProbeRuntime {
        ffprobe: Path::new(&args[2]),
        root: Path::new(&args[3]),
        timeout: Duration::from_secs(300),
        max_source_bytes: 1024 * 1024 * 1024,
    };
    let started = Instant::now();
    let original = probe(Path::new(&args[0]), &runtime, &flag)?;
    let video = PreparedVideo::open(Path::new(&args[0]), &original, &runtime, &flag)?;
    let prepare_seconds = started.elapsed().as_secs_f64();
    let s = &original.metadata.video;
    let frame_bytes = u64::from(s.width) * u64::from(s.height) * 4;
    if video.frame_count() == 0 || video.frame_count() as u64 * frame_bytes > 4 * 1024 * 1024 * 1024
    {
        return Err(
            "benchmark requires a nonempty source within the 4GiB range output budget".into(),
        );
    }
    let batch = (64 * 1024 * 1024 / frame_bytes).clamp(1, 16) as usize;
    let continuous = measure(&video, Path::new(&args[1]), video.frame_count(), &flag)?;
    eprintln!(
        "Continuous pass complete: {} seconds",
        continuous["seconds"]
    );
    let batched = measure(&video, Path::new(&args[1]), batch, &flag)?;
    assert_eq!(continuous["rgba_sha256"], batched["rgba_sha256"]);
    assert_eq!(continuous["frames"], batched["frames"]);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"source":args[0],
        "probe":original,"prepare_seconds":prepare_seconds,"continuous":continuous,"batched":batched,
        "pixels_equal":true,"scope":"decode only, no compositor/encoder/native GUI"}))?
    );
    Ok(())
}
