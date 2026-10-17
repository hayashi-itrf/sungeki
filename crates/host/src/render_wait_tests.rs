use super::*;
use crate::render_record::{Input, State};
use std::{fs, path::PathBuf, thread};
use sungeki_project::JobPhase;
use sungeki_protocol::{RenderJobRequest, RenderPurpose, SessionRequest};
use sungeki_storage::{JobKind, StoredJob};

fn fixture() -> (PathBuf, Arc<Mutex<Sessions>>, String) {
    let root = std::env::temp_dir().join(format!("render-wait-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "wait fixture".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let input = Input {
        purpose: RenderPurpose::Final,
        parent_job_id: None,
        source_revision: snapshot.revision,
        content_hash: sungeki_storage::project_hash(&snapshot.project).unwrap(),
        project: snapshot.project,
        directory: root.join("output"),
        runtime: sungeki_render::RenderRuntime {
            vrm: None,
            python: "/unused".into(),
            image_worker: "/unused".into(),
            ffmpeg: "/unused".into(),
            ffprobe: "/unused".into(),
            font: "/unused".into(),
            font_sha256: "0".repeat(64),
            deadline: Duration::from_secs(1),
        },
    };
    let id = uuid::Uuid::new_v4().to_string();
    sessions
        .store
        .create_job(&StoredJob {
            id: id.clone(),
            kind: JobKind::Render,
            session_id: snapshot.session_id,
            input: serde_json::to_vec(&input).unwrap(),
            version: 0,
            state: serde_json::to_vec(&State::queued()).unwrap(),
        })
        .unwrap();
    (root, Arc::new(Mutex::new(sessions)), id)
}

#[test]
fn render_wait_timeout_keeps_same_job_and_durable_state() {
    let (root, shared, id) = fixture();
    let before = shared.lock().unwrap().store.load_job(&id).unwrap();
    let started = Instant::now();
    let SessionResponse::RenderJob { job } = get(&shared, &id, 50).unwrap() else {
        panic!()
    };
    assert!(started.elapsed() >= Duration::from_millis(50));
    assert_eq!(job.job_id, id);
    assert_eq!(job.phase, JobPhase::Queued);
    assert_eq!(shared.lock().unwrap().store.load_job(&id).unwrap(), before);
    assert_eq!(
        get(&shared, &id, 30_001).unwrap_err().code,
        ErrorCode::ValidationFailed
    );
    assert_eq!(
        get(&shared, "missing", 30_000).unwrap_err().code,
        ErrorCode::NotFound
    );
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn render_wait_allows_other_requests_and_wakes_all_readers_after_cancel() {
    let (root, shared, id) = fixture();
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let shared = shared.clone();
            let id = id.clone();
            thread::spawn(move || get(&shared, &id, 5_000).unwrap())
        })
        .collect();
    thread::sleep(Duration::from_millis(50));
    // A regular common-host request remains available while readers are waiting.
    Sessions::execute(
        &shared,
        SessionRequest::Create {
            title: "concurrent editing".into(),
        },
    )
    .unwrap();
    let cancelled_at = Instant::now();
    crate::render_api::execute(&shared, RenderJobRequest::Cancel { job_id: id.clone() }).unwrap();
    {
        let mut sessions = shared.lock().unwrap();
        let row = crate::render_record::record(&sessions, &id).unwrap();
        let (_, mut state) = crate::render_record::decode(&row).unwrap();
        let mut lifecycle = state.lifecycle().unwrap();
        lifecycle.acknowledge_cancel().unwrap();
        state.update(lifecycle);
        crate::render_record::save(&mut sessions, &row, &state).unwrap();
    }
    for reader in readers {
        let SessionResponse::RenderJob { job } = reader.join().unwrap() else {
            panic!()
        };
        assert_eq!(job.phase, JobPhase::Cancelled);
        assert_eq!(job.job_id, id);
    }
    assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    let started = Instant::now();
    get(&shared, &id, 30_000).unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stage_updates_are_durable_and_cannot_overwrite_cancellation() {
    use crate::render_record::{decode, record, save};
    use crate::render_worker::progress::update;
    use sungeki_render::RenderProgress;
    let (root, shared, id) = fixture();
    {
        let mut sessions = shared.lock().unwrap();
        let row = record(&sessions, &id).unwrap();
        let (_, mut state) = decode(&row).unwrap();
        let mut lifecycle = state.lifecycle().unwrap();
        lifecycle.start().unwrap();
        state.update(lifecycle);
        save(&mut sessions, &row, &state).unwrap();
    }
    update(
        &shared,
        &id,
        RenderProgress::Drawing {
            completed: 32,
            total: 100,
        },
    )
    .unwrap();
    let SessionResponse::RenderJob { job } = get(&shared, &id, 0).unwrap() else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Running);
    assert!(job.output.is_none());
    let progress = job.progress.unwrap();
    assert!(progress.stage.contains("32 / 100"));
    assert_eq!(progress.fraction, None);
    let row = shared.lock().unwrap().store.load_job(&id).unwrap().unwrap();
    assert_eq!(
        decode(&row).unwrap().1.progress.unwrap().stage,
        progress.stage
    );
    crate::render_api::execute(&shared, RenderJobRequest::Cancel { job_id: id.clone() }).unwrap();
    let before = shared.lock().unwrap().store.load_job(&id).unwrap();
    update(&shared, &id, RenderProgress::Verifying).unwrap();
    assert_eq!(shared.lock().unwrap().store.load_job(&id).unwrap(), before);
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}
