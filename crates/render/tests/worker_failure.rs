#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_project::{Project, Scene, TimeUs};
use sungeki_render::{RenderPlan, RenderRuntime, render_staged};

#[test]
fn failed_worker_cancel_and_nonempty_staging_never_return_an_artifact() {
    let root = std::env::temp_dir().join(format!("vvm-render-failure-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let font = root.join("font");
    fs::write(&font, b"captured-font-test").unwrap();
    let font_sha256 = format!("{:x}", Sha256::digest(b"captured-font-test"));
    let runtime = RenderRuntime {
        vrm: None,
        python: "/usr/bin/false".into(),
        image_worker: "/unused.py".into(),
        ffmpeg: PathBuf::from("/unused-ffmpeg"),
        ffprobe: "/unused-ffprobe".into(),
        font,
        font_sha256,
        deadline: Duration::from_secs(5),
    };
    let mut project = Project::empty(uuid::Uuid::new_v4().to_string(), "failure test".into());
    project.scenes.push(Scene {
        fade: None,
        id: uuid::Uuid::new_v4().to_string(),
        name: "test".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(100000),
    });
    let plan = RenderPlan::new(&project, |_| Ok(())).unwrap();
    let work = root.join("work");
    fs::create_dir(&work).unwrap();
    let mut observations = Vec::new();
    assert!(
        render_staged(
            &plan,
            &runtime,
            &work,
            &AtomicBool::new(true),
            &mut |event| observations.push(event)
        )
        .is_err()
    );
    assert!(observations.is_empty());
    assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
    let failure = render_staged(
        &plan,
        &runtime,
        &work,
        &AtomicBool::new(false),
        &mut |event| observations.push(event),
    )
    .unwrap_err();
    assert!(failure.to_string().contains("外部描画処理"), "{failure}");
    assert!(!work.join("output.mp4").exists());
    assert!(
        observations.contains(&sungeki_render::RenderProgress::Drawing {
            completed: 0,
            total: plan.frame_count()
        })
    );
    assert!(!observations.contains(&sungeki_render::RenderProgress::Verifying));
    let captured = fs::read(work.join("font.ttc")).unwrap();
    let failure =
        render_staged(&plan, &runtime, &work, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
    assert!(failure.to_string().contains("空の専用作業"), "{failure}");
    assert_eq!(fs::read(work.join("font.ttc")).unwrap(), captured);
    fs::remove_dir_all(root).unwrap();
}
