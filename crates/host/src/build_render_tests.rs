use super::*;
use crate::build_record::{Reservation, reserve};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{RenderJobRequest, SessionResponse, StartRenderArgs};

pub(super) async fn ready() -> (
    std::path::PathBuf,
    Arc<Mutex<Sessions>>,
    StoredJob,
    StoredJob,
) {
    ready_with_child(|_, _| {}).await
}

pub(super) async fn ready_with_child(
    change: impl FnOnce(&mut Sessions, &mut StoredJob),
) -> (
    std::path::PathBuf,
    Arc<Mutex<Sessions>>,
    StoredJob,
    StoredJob,
) {
    let (root, shared, row) = super::save_tests::ready().await;
    crate::build_record::save_project(&shared, &row).unwrap();
    let (parent, child) = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&row.id).unwrap().unwrap();
        let Reservation::New(parent) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        let (_, state) = decode(&parent).unwrap();
        let (_, current) = sessions.load("session").unwrap();
        let project = current.history().project().clone();
        let input = crate::render_record::Input {
            purpose: sungeki_protocol::RenderPurpose::Final,
            parent_job_id: Some(parent.id.clone()),
            source_revision: state.expected_revision,
            content_hash: sungeki_storage::project_hash(&project).unwrap(),
            project,
            directory: root.join("must-not-render"),
            runtime: sungeki_render::RenderRuntime {
                vrm: None,

                ffmpeg: "/unavailable/ffmpeg".into(),
                ffprobe: "/unavailable/ffprobe".into(),
                font: "/unavailable/font".into(),
                font_sha256: "0".repeat(64),
                deadline: std::time::Duration::from_secs(60),
            },
        };
        let mut child = StoredJob {
            id: state.child_id.unwrap(),
            kind: JobKind::Render,
            session_id: parent.session_id.clone(),
            version: 0,
            input: serde_json::to_vec(&input).unwrap(),
            state: serde_json::to_vec(&crate::render_record::State::queued()).unwrap(),
        };
        change(&mut sessions, &mut child);
        sessions.store.create_job(&child).unwrap();
        (parent, child)
    };
    (root, shared, parent, child)
}
#[tokio::test]
async fn parent_render_replay_does_not_launch_and_cannot_be_taken_over_as_standalone() {
    let (root, shared, parent, child) = ready().await;
    for _ in 0..2 {
        let response = crate::build_record::start_render(&shared, &parent).unwrap();
        assert!(
            matches!(response,SessionResponse::RenderJob {job} if job.job_id==child.id && job.phase==JobPhase::Queued)
        );
    }
    let (_, state) = decode(&parent).unwrap();
    assert!(
        crate::render_api::execute(
            &shared,
            RenderJobRequest::Start {
                args: StartRenderArgs {
                    wait_ms: None,
                    session_id: parent.session_id.clone(),
                    job_id: child.id.clone(),
                    expected_revision: state.expected_revision
                }
            }
        )
        .is_err()
    );
    let sessions = shared.lock().unwrap();
    assert_eq!(sessions.render_runtime.active.load(Ordering::SeqCst), 0);
    assert_eq!(sessions.store.load_job(&child.id).unwrap().unwrap(), child);
    assert!(!root.join("must-not-render").exists());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn queued_render_rechecks_parent_cancellation_before_any_worker_io() {
    let (root, shared, parent, child) = ready().await;
    let runtime = {
        let mut sessions = shared.lock().unwrap();
        crate::build_record::request_cancel(&mut sessions, &parent).unwrap();
        sessions.render_runtime.clone()
    };
    assert!(crate::build_record::start_render(&shared, &parent).is_err());
    runtime.active.fetch_add(1, Ordering::SeqCst);
    crate::render_worker::launch(
        shared.clone(),
        runtime.clone(),
        child.id.clone(),
        Arc::new(AtomicBool::new(false)),
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let sessions = shared.lock().unwrap();
    let (_, state) =
        crate::render_record::decode(&sessions.store.load_job(&child.id).unwrap().unwrap())
            .unwrap();
    assert_eq!(state.phase, JobPhase::Cancelled);
    assert!(state.output.is_none());
    assert!(!root.join("must-not-render").exists());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn parent_cancellation_signals_owned_render_without_claiming_it_has_stopped() {
    let (root, shared, parent, child) = ready().await;
    let flag = Arc::new(AtomicBool::new(false));
    {
        let mut sessions = shared.lock().unwrap();
        sessions
            .render_runtime
            .flags
            .lock()
            .unwrap()
            .insert(child.id.clone(), flag.clone());
        let cancelling = crate::build_record::request_cancel(&mut sessions, &parent).unwrap();
        assert!(flag.load(Ordering::SeqCst));
        assert_eq!(decode(&cancelling).unwrap().1.phase, JobPhase::Cancelling);
        assert_eq!(sessions.store.load_job(&child.id).unwrap().unwrap(), child);
        sessions
            .render_runtime
            .flags
            .lock()
            .unwrap()
            .remove(&child.id);
    }
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
