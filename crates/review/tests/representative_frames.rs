#![cfg(unix)]
use std::{fs, sync::atomic::AtomicBool, time::Duration};
use sungeki_project::{CheckStatus, Project, Scene, TimeUs};
use sungeki_render::RenderRuntime;
use sungeki_review::{RepresentativeFrameError, review_representative_frames};
#[test]
fn cancelled_or_nonempty_work_is_protected_and_each_frame_failure_is_reported() {
    let root = std::env::temp_dir().join(format!("vvm-representative-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut p = Project::empty(uuid::Uuid::new_v4().to_string(), "画像確認".into());
    p.scenes.push(Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name: "一場面".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(100_000),
        fade: None,
    });
    let runtime = RenderRuntime {
        vrm: None,

        ffmpeg: "/unused".into(),
        ffprobe: "/unused".into(),
        font: "/missing-font".into(),
        font_sha256: "0".repeat(64),
        deadline: Duration::from_secs(5),
    };
    assert!(matches!(
        review_representative_frames(&p, &[], 3, 0, &runtime, &root, &AtomicBool::new(true)),
        Err(RepresentativeFrameError::Cancelled)
    ));
    fs::write(root.join("keep"), b"preserved").unwrap();
    assert!(
        review_representative_frames(&p, &[], 3, 0, &runtime, &root, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(fs::read(root.join("keep")).unwrap(), b"preserved");
    fs::remove_file(root.join("keep")).unwrap();
    let report =
        review_representative_frames(&p, &[], 3, 0, &runtime, &root, &AtomicBool::new(false))
            .unwrap();
    assert_eq!(report.frames.len(), 2); // Scene midpoint rounds to the last frame (2).
    assert!(
        report
            .frames
            .iter()
            .all(|f| f.output.is_none() && f.error.is_some())
    );
    assert_eq!(report.check.status, CheckStatus::Failed);
    assert_eq!(report.visual_review.status, CheckStatus::NotRun);
    assert_eq!(report.contact_sheets.len(), 1);
    assert!(report.contact_sheets[0].output.is_none());
    assert!(report.contact_sheets[0].error.is_some());
    fs::remove_dir_all(&root).unwrap();
    fs::create_dir(&root).unwrap();
    p.scenes = (0..7)
        .map(|index| Scene {
            id: uuid::Uuid::new_v4().to_string(),
            name: "page test".into(),
            start_us: TimeUs(index * 1_000_000),
            duration_us: TimeUs(1_000_000),
            fade: None,
        })
        .collect();
    let paged =
        review_representative_frames(&p, &[], 13, 0, &runtime, &root, &AtomicBool::new(false))
            .unwrap();
    assert_eq!(paged.contact_sheets.len(), 2);
    assert_eq!(paged.contact_sheets[0].frame_indices.len(), 12);
    assert_eq!(paged.contact_sheets[1].frame_indices.len(), 1);
    assert_eq!(
        paged
            .contact_sheets
            .iter()
            .flat_map(|p| p.frame_indices.iter().copied())
            .collect::<Vec<_>>(),
        paged
            .frames
            .iter()
            .map(|f| f.sample.frame_index)
            .collect::<Vec<_>>()
    );
    assert_eq!(paged.check.status, CheckStatus::Failed);
    fs::remove_dir_all(root).unwrap();
}
