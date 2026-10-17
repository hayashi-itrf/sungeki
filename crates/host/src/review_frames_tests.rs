use super::*;
use crate::review_record::{State, VisualInput, decode};
use std::time::Duration;
use sungeki_project::{JobPhase, Project, Scene, TimeUs};
use sungeki_protocol::FramePreviewOutput;
use sungeki_storage::{JobKind, StoredJob};
#[test]
fn report_preserves_partial_failures_and_rejects_foreign_paths_or_false_coverage() {
    let id = uuid::Uuid::new_v4().to_string();
    let root = std::env::temp_dir().join(format!("vvm-review-frame-{id}"));
    fs::create_dir(&root).unwrap();
    let mut project = Project::empty(uuid::Uuid::new_v4().to_string(), "確認記録".into());
    project.scenes.push(Scene {
        id: uuid::Uuid::new_v4().to_string(),
        name: "test".into(),
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        fade: None,
    });
    let input = Input {
        sampling: Default::default(),
        parent_job_id: None,
        source_revision: 0,
        content_hash: sungeki_storage::project_hash(&project).unwrap(),
        project: project.clone(),
        visual_unavailable: None,
        visual: Some(VisualInput {
            directory: root.join(format!("{id}.frames")),
            runtime: sungeki_render::RenderRuntime {
                vrm: None,
                python: "/usr/bin/false".into(),
                image_worker: "/unused".into(),
                ffmpeg: "/unused".into(),
                ffprobe: "/unused".into(),
                font: "/missing-font".into(),
                font_sha256: "0".repeat(64),
                deadline: Duration::from_secs(5),
            },
        }),
    };
    let report = complete(
        &input,
        &AtomicBool::new(false),
        ReviewReport {
            report_id: id.clone(),
            source_revision: 0,
            content_hash: input.content_hash.clone(),
            asset_fingerprint: None,
            diagnostics: sungeki_review::review_structure(&project),
            representative_frames: None,
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(report.diagnostics.summary.failed, 1);
    assert!(!report.diagnostics.summary.complete);
    assert!(
        report
            .representative_frames
            .as_ref()
            .unwrap()
            .frames
            .iter()
            .all(|f| f.error.is_some())
    );
    assert!(!crate::review_jobs::has_outdated_images(None));
    assert!(!crate::review_jobs::has_outdated_images(Some(&report)));
    let state = State {
        phase: JobPhase::Succeeded,
        progress: None,
        report: Some(report),
        error: None,
    };
    let mut job = StoredJob {
        id,
        kind: JobKind::Review,
        session_id: "fixture".into(),
        input: serde_json::to_vec(&input).unwrap(),
        version: 0,
        state: serde_json::to_vec(&state).unwrap(),
    };
    let (_, mut state) = decode(&job).unwrap();
    let frames = state
        .report
        .as_mut()
        .unwrap()
        .representative_frames
        .as_mut()
        .unwrap();
    let frame = &mut frames.frames[0];
    frame.error = None;
    frame.output = Some(FramePreviewOutput {
        path: input
            .visual
            .as_ref()
            .unwrap()
            .directory
            .join(format!(
                "frame-{:012}/frame-0.png",
                frame.sample.frame_index
            ))
            .to_str()
            .unwrap()
            .into(),
        sha256: "1".repeat(64),
        byte_length: 12,
        width: project.canvas.width,
        height: project.canvas.height,
        frame_index: frame.sample.frame_index,
        time_us: frame.sample.time_us,
        font_sha256: "0".repeat(64),
        layout_version: 8,
    });
    job.state = serde_json::to_vec(&state).unwrap();
    decode(&job).unwrap();
    // Stored images keep their recorded layout; new renders use version 9.
    for (version, accepted) in [(7, true), (8, true), (9, true), (0, false), (10, false)] {
        state
            .report
            .as_mut()
            .unwrap()
            .representative_frames
            .as_mut()
            .unwrap()
            .frames[0]
            .output
            .as_mut()
            .unwrap()
            .layout_version = version;
        job.state = serde_json::to_vec(&state).unwrap();
        assert_eq!(decode(&job).is_ok(), accepted);
        if accepted {
            assert_eq!(
                crate::review_jobs::has_outdated_images(state.report.as_ref()),
                version != sungeki_render::CURRENT_LAYOUT_VERSION,
                "saved renderer version {version}"
            );
        }
    }
    state
        .report
        .as_mut()
        .unwrap()
        .representative_frames
        .as_mut()
        .unwrap()
        .frames[0]
        .output
        .as_mut()
        .unwrap()
        .layout_version = 8;
    job.state = serde_json::to_vec(&state).unwrap();
    let valid = job.state.clone();
    state
        .report
        .as_mut()
        .unwrap()
        .representative_frames
        .as_mut()
        .unwrap()
        .frames[0]
        .output
        .as_mut()
        .unwrap()
        .path = "/foreign/frame.png".into();
    job.state = serde_json::to_vec(&state).unwrap();
    assert!(decode(&job).is_err());
    job.state = valid;
    let (_, mut state) = decode(&job).unwrap();
    state
        .report
        .as_mut()
        .unwrap()
        .representative_frames
        .as_mut()
        .unwrap()
        .sampling
        .unsampled
        .clear();
    job.state = serde_json::to_vec(&state).unwrap();
    assert!(decode(&job).is_err());
    fs::remove_dir_all(root).unwrap();
}
