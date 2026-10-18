use sha2::{Digest, Sha256};
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use sungeki_project::{AssetLocator, Project};
use sungeki_render::{RenderPlan, RenderRuntime, render_staged};

#[test]
fn missing_video_reports_the_asset_and_original_without_producing_output() {
    let root = std::env::temp_dir().join(format!("vvm-missing-video-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let font = root.join("font");
    fs::write(&font, b"font").unwrap();
    let mut project: Project = serde_json::from_str(include_str!(
        "../../../tests/fixtures/video-clip-project.json"
    ))
    .unwrap();
    let missing = root.join("missing-original.mp4");
    project.assets[0].name = "会話の背景動画".into();
    project.assets[0].locator = AssetLocator::Absolute {
        path: missing.to_str().unwrap().into(),
    };
    let before = project.clone();
    let plan = RenderPlan::new(&project, |_| Ok(())).unwrap();
    let work = root.join("work");
    fs::create_dir(&work).unwrap();
    let runtime = RenderRuntime {
        vrm: None,

        ffmpeg: "/unused-ffmpeg".into(),
        ffprobe: "/unused-ffprobe".into(),
        font,
        font_sha256: format!("{:x}", Sha256::digest(b"font")),
        deadline: Duration::from_secs(5),
    };
    let failure = render_staged(&plan, &runtime, &work, &AtomicBool::new(false), &mut |_| {})
        .unwrap_err()
        .to_string();
    assert!(failure.contains("会話の背景動画"), "{failure}");
    assert!(failure.contains(&project.assets[0].id), "{failure}");
    assert!(failure.contains(missing.to_str().unwrap()), "{failure}");
    assert!(failure.contains("保存場所と内容を確認"), "{failure}");
    assert!(!work.join("output.mp4").exists());
    assert_eq!(project, before);
    fs::remove_dir_all(root).unwrap();
}
