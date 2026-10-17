use super::*;
use crate::RenderRuntime;
use std::{
    fs,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use sungeki_project::Project;

#[test]
fn whole_batch_validation_precedes_cache_commit() {
    for case in [
        "valid",
        "count",
        "version",
        "name",
        "time",
        "font",
        "overflow",
        "missing",
        "cancel",
        "exit",
        "worker_cancel",
    ] {
        let directory = std::env::temp_dir().join(format!("vvm-batch-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let script = directory.join("worker.sh");
        fs::write(
            &script,
            if case == "exit" {
                "exit 1"
            } else if case == "worker_cancel" {
                "sleep 5; cat receipt.json"
            } else {
                "cat receipt.json"
            },
        )
        .unwrap();
        let runtime = RenderRuntime {
            vrm: None,
            python: "/bin/sh".into(),
            image_worker: script,
            ffmpeg: "/unused".into(),
            ffprobe: "/unused".into(),
            font: "/unused".into(),
            font_sha256: "font-a".into(),
            deadline: Duration::from_secs(5),
        };
        let flag = std::sync::Arc::new(AtomicBool::new(case == "cancel"));
        let runner = Runner {
            runtime: (&runtime).into(),
            directory: &directory,
            flag: &flag,
            end: Instant::now() + Duration::from_secs(5),
        };
        let mut canvas = Project::empty("test".into(), "test".into()).canvas;
        let png = include_bytes!("../../../tests/fixtures/build-source.png");
        canvas.width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        canvas.height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        let mut cache = FrameCache::batched();
        let mut results = Vec::new();
        for i in 0..2 {
            cache.frame(&runner, &canvas, &[], i, i as u64).unwrap();
            if case != "missing" || i == 0 {
                fs::write(directory.join(format!("frame-{i}.png")), png).unwrap();
            }
            results.push(json!({"output":format!("frame-{i}.png"), "report": {
                "layout_version":9, "font_sha256":"font-a", "frame_index":i,
                "time_us":canvas.fps.frame_time(i as u64), "issues":[]
            }}));
        }
        assert!(cache.frames.is_empty());
        match case {
            "count" => {
                results.pop();
            }
            "name" => results[1]["output"] = json!("frame-0.png"),
            "time" => results[1]["report"]["time_us"] = json!(999),
            "font" => results[1]["report"]["font_sha256"] = json!("other"),
            "overflow" => results[1]["report"]["issues"] = json!([{"code":"TEXT_OVERFLOW"}]),
            _ => {}
        }
        fs::write(
            directory.join("receipt.json"),
            serde_json::to_vec(&json!({
                "version":if case == "version" { 2 } else { 1 }, "frames":results
            }))
            .unwrap(),
        )
        .unwrap();
        let stopper = if case == "worker_cancel" {
            let flag = flag.clone();
            Some(std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }))
        } else {
            None
        };
        let result = cache.flush(&runner, &canvas);
        if let Some(stopper) = stopper {
            stopper.join().unwrap();
        }

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
            assert!(cache.frames.is_empty(), "{case}");
            assert_eq!(cache.stats.rendered, 0, "{case}");
            assert!(!directory.join("report-0.json").exists(), "{case}");
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
