use crate::{
    RenderRuntime,
    execute::{Runner, strings},
    video_segments::VideoSegments,
};
use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use sungeki_project::FrameRate;

#[test]
#[ignore = "requires real FFmpeg and ffprobe"]
fn real_batched_video_preserves_boundaries_rates_and_cancel() {
    let root = std::env::temp_dir().join(format!("vvm-segments-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let runtime = RenderRuntime {
        vrm: None,

        font: "/unused".into(),
        font_sha256: String::new(),
        ffmpeg: "/opt/homebrew/bin/ffmpeg".into(),
        ffprobe: "/opt/homebrew/bin/ffprobe".into(),
        deadline: Duration::from_secs(60),
    };
    let flag = AtomicBool::new(false);
    for denominator in [1, 1001] {
        let dir = root.join(denominator.to_string());
        fs::create_dir(&dir).unwrap();
        let runner = Runner {
            runtime: (&runtime).into(),
            directory: &dir,
            flag: &flag,
            end: Instant::now() + Duration::from_secs(60),
        };
        for color in ["black", "white"] {
            runner
                .ffmpeg(strings(&[
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("color={color}:s=16x16"),
                    "-frames:v",
                    "1",
                    &format!("{color}.png"),
                ]))
                .unwrap();
        }
        let fps = FrameRate {
            numerator: if denominator == 1 { 30 } else { 30000 },
            denominator,
        };
        let mut batch = VideoSegments::new(&fps);
        for (first, end, color) in [
            (0, 3, "black"),
            (3, 119, "white"),
            (119, 120, "black"),
            (120, 123, "white"),
            (123, 244, "white"),
            (244, 245, "black"),
        ] {
            batch
                .push(&runner, &dir.join(format!("{color}.png")), first, end)
                .unwrap();
        }
        let concat = batch.finish(&runner, 245).unwrap();
        assert_eq!(concat.matches("file '").count(), 4);
        assert!(!dir.join("sequence-0").exists());
        assert!(!dir.join("sequence-3").exists());
        fs::write(dir.join("video.ffconcat"), concat).unwrap();
        runner
            .ffmpeg(strings(&[
                "-f",
                "concat",
                "-safe",
                "1",
                "-i",
                "video.ffconcat",
                "-c",
                "copy",
                "-video_track_timescale",
                &fps.numerator.to_string(),
                "output.mp4",
            ]))
            .unwrap();
        let probe = runner
            .run(
                &runtime.ffprobe,
                strings(&["-v", "error", "-show_streams", "-of", "json", "output.mp4"]),
            )
            .unwrap();
        let p: serde_json::Value = serde_json::from_slice(&probe).unwrap();
        assert_eq!(p["streams"][0]["nb_frames"], "245");
        let seconds = p["streams"][0]["duration"]
            .as_str()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!((seconds - 245. * denominator as f64 / fps.numerator as f64).abs() < 0.00001);
        let rgb = runner
            .run(
                &runtime.ffmpeg,
                strings(&[
                    "-v",
                    "error",
                    "-i",
                    "output.mp4",
                    "-pix_fmt",
                    "rgb24",
                    "-f",
                    "rawvideo",
                    "pipe:1",
                ]),
            )
            .unwrap();
        assert_eq!(rgb.len(), 245 * 16 * 16 * 3);
        for (i, frame) in rgb.chunks_exact(16 * 16 * 3).enumerate() {
            let value = if i < 3 || i == 119 || i == 244 {
                0
            } else {
                255
            };
            assert!(
                frame.iter().all(|&v| v == value),
                "wrong pixels at frame {i}"
            );
        }
        let mut invalid = VideoSegments::new(&fps);
        assert!(invalid.push(&runner, &dir.join("black.png"), 1, 2).is_err());
        invalid.push(&runner, &dir.join("black.png"), 0, 1).unwrap();
        assert!(invalid.finish(&runner, 2).is_err());
        let mut cancel = VideoSegments::new(&fps);
        cancel.push(&runner, &dir.join("black.png"), 0, 1).unwrap();
        flag.store(true, Ordering::SeqCst);
        assert!(cancel.finish(&runner, 1).is_err());
        assert!(dir.join("black.png").exists());
        flag.store(false, Ordering::SeqCst);
    }
    fs::remove_dir_all(root).unwrap();
}
