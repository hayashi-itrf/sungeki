use super::*;
use std::time::Duration;
use sungeki_project::{AssetProbe, ClipContent, Scene};

fn empty_scene() -> Project {
    let mut project = Project::empty(uuid::Uuid::new_v4().to_string(), "frame test".into());
    project.scenes.push(Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name: "still".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(100000),
        fade: None,
    });
    project
}

#[test]
fn stills_allow_unsynthesized_captions_without_weakening_video_preflight() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scene-evaluation.json"
    ))
    .unwrap();
    let mut project: Project = serde_json::from_value(data["project"].clone()).unwrap();
    for clip in &mut project.clips {
        if let ClipContent::Speech { audio, .. } = &mut clip.content {
            *audio = None;
        }
    }
    let original = project.clone();
    assert!(RenderPlan::new(&project, |_| Ok(())).is_err());
    let frame = FramePlan::new(&project, 0).unwrap();
    assert!(frame.render.scene.audio.is_empty());
    assert!(
        frame
            .render
            .sources
            .values()
            .all(|a| !matches!(a.probe, AssetProbe::Audio(_)))
    );
    assert_eq!(project, original);
}

#[test]
fn frame_bounds_are_half_open_and_invalid_projects_stay_invalid() {
    let mut project = empty_scene();
    assert!(FramePlan::new(&project, 2).is_ok());
    assert!(FramePlan::new(&project, 3).is_err());
    assert!(FramePlan::new(&project, u64::MAX).is_err());
    project.canvas.fps.numerator = 0;
    assert!(FramePlan::new(&project, 0).is_err());
}

#[test]
fn still_prepares_only_visible_vrm_frames_without_changing_movie_clock() {
    let mut project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/vrm-clip-project.json"
    ))
    .unwrap();
    project.scenes[0].duration_us = TimeUs(2_000_000);
    let original = project.clone();
    let movie = sungeki_scene::ScenePlan::new(&project).unwrap();
    assert_eq!(movie.layers.len(), 30);
    for index in [0, 15, 29, 30, 59] {
        let frame = FramePlan::new(&project, index).unwrap();
        let expected: Vec<_> = movie.frame(index).unwrap().into_iter().cloned().collect();
        assert_eq!(frame.render.scene.layers, expected);
        assert_eq!(frame.render.scene.frame_count, movie.frame_count);
        assert_eq!(frame.render.scene.duration_us, movie.duration_us);
        assert_eq!(frame.render.sources.len(), if index < 30 { 2 } else { 0 });
        assert!(frame.render.scene.layers.len() <= 1);
    }
    assert_eq!(project, original);
}

#[test]
fn cancellation_and_nonempty_directory_never_replace_files() {
    let directory = std::env::temp_dir().join(format!("vvm-frame-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let runtime = RenderRuntime {
        vrm: None,
        python: "/usr/bin/false".into(),
        image_worker: "/unused.py".into(),
        ffmpeg: "/unused".into(),
        ffprobe: "/unused".into(),
        font: "/unused".into(),
        font_sha256: "a".repeat(64),
        deadline: Duration::from_secs(5),
    };
    let plan = FramePlan::new(&empty_scene(), 0).unwrap();
    let error =
        render_frame_staged(&plan, &runtime, &directory, &AtomicBool::new(true)).unwrap_err();
    assert!(error.to_string().contains("取り消"));
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    fs::write(directory.join("frame-0.png"), b"existing").unwrap();
    assert!(render_frame_staged(&plan, &runtime, &directory, &AtomicBool::new(false)).is_err());
    assert_eq!(
        fs::read(directory.join("frame-0.png")).unwrap(),
        b"existing"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn invalid_png_is_rejected_even_when_worker_reports_success() {
    use sha2::{Digest, Sha256};
    let root = std::env::temp_dir().join(format!("vvm-frame-invalid-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let work = root.join("work");
    fs::create_dir(&work).unwrap();
    let font = root.join("font");
    fs::write(&font, b"fixture-font").unwrap();
    let hash = format!("{:x}", Sha256::digest(b"fixture-font"));
    let worker = root.join("worker.sh");
    let report = serde_json::json!({"issues":[],"layout_version":9,
        "font_sha256":hash,"frame_index":0,"time_us":0});
    fs::write(
        &worker,
        format!("printf 'not a PNG' > frame-0.png\nprintf '%s' '{report}'\n"),
    )
    .unwrap();
    let runtime = RenderRuntime {
        vrm: None,
        python: "/bin/sh".into(),
        image_worker: worker,
        ffmpeg: "/unused".into(),
        ffprobe: "/unused".into(),
        font,
        font_sha256: hash,
        deadline: Duration::from_secs(5),
    };
    let error = render_frame_staged(
        &FramePlan::new(&empty_scene(), 0).unwrap(),
        &runtime,
        &work,
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(!error.to_string().contains("外部描画処理"), "{error}");
    assert_eq!(fs::read(work.join("frame-0.png")).unwrap(), b"not a PNG");
    fs::remove_dir_all(root).unwrap();
}
