use super::*;
use crate::RenderRuntime;
use std::{
    fs,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use sungeki_project::Project;

#[test]
fn native_batch_commits_only_after_all_frames_succeed() {
    for case in [
        "valid",
        "existing-output",
        "bad-resources",
        "changed-font",
        "cancel",
    ] {
        let directory =
            std::env::temp_dir().join(format!("sungeki-native-batch-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        // Empty canvas does not read glyphs, but still authenticates its captured font.
        fs::write(directory.join("font.ttc"), b"fixture-font").unwrap();
        fs::write(directory.join("resources.json"), b"{}").unwrap();
        let runtime = RenderRuntime {
            vrm: None,
            ffmpeg: "/unused".into(),
            ffprobe: "/unused".into(),
            font: "/unused".into(),
            font_sha256: format!("{:x}", Sha256::digest(b"fixture-font")),
            deadline: Duration::from_secs(5),
        };
        let flag = AtomicBool::new(false);
        let runner = Runner {
            runtime: (&runtime).into(),
            directory: &directory,
            flag: &flag,
            end: Instant::now() + Duration::from_secs(5),
        };
        let mut canvas = Project::empty("test".into(), "test".into()).canvas;
        canvas.width = 8;
        canvas.height = 8;
        let mut cache = FrameCache::batched();
        for i in 0..2 {
            cache.frame(&runner, &canvas, &[], i, i as u64).unwrap();
        }
        match case {
            "existing-output" => fs::write(directory.join("frame-1.png"), b"untouched").unwrap(),
            "bad-resources" => fs::write(directory.join("resources.json"), b"bad json").unwrap(),
            "changed-font" => fs::write(directory.join("font.ttc"), b"changed").unwrap(),
            "cancel" => flag.store(true, std::sync::atomic::Ordering::Relaxed),
            _ => {}
        }
        let result = cache.flush(&runner, &canvas);
        if case == "valid" {
            result.unwrap();
            assert_eq!(cache.stats.rendered, 2);
            assert!(cache.pending.is_empty());
            assert_eq!(
                cache.frame(&runner, &canvas, &[], 2, 2).unwrap(),
                directory.join("frame-1.png")
            );
            assert_eq!(cache.stats.reused, 1);
        } else {
            assert!(result.is_err(), "{case}");
            assert!(cache.frames.is_empty());
            assert_eq!(cache.stats.rendered, 0);
            assert!(!directory.join("report-0.json").exists());
            if case == "existing-output" {
                assert_eq!(
                    fs::read(directory.join("frame-1.png")).unwrap(),
                    b"untouched"
                );
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
