use crate::{
    render_record::{Input, State, decode},
    sessions::Sessions,
};
use std::{fs, time::Duration};
use sungeki_project::JobPhase;
use sungeki_protocol::{SessionRequest, SessionResponse};
use sungeki_storage::{JobKind, StoredJob};

#[test]
fn interrupted_render_keeps_its_work_and_never_claims_a_completed_output() {
    let root = std::env::temp_dir().join(format!("vvm-render-recovery-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let db = root.join("db");
    let mut sessions = Sessions::open(&db).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "中断復旧".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let directory = root.join("owned-work");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("partial.mp4"), b"unverified partial").unwrap();
    let input = Input {
        purpose: sungeki_protocol::RenderPurpose::Final,
        parent_job_id: None,
        source_revision: 0,
        content_hash: sungeki_storage::project_hash(&snapshot.project).unwrap(),
        project: snapshot.project,
        directory: directory.clone(),
        runtime: sungeki_render::RenderRuntime {
            vrm: None,
            python: "/unavailable/python".into(),
            image_worker: "/unavailable/worker".into(),
            ffmpeg: "/unavailable/ffmpeg".into(),
            ffprobe: "/unavailable/ffprobe".into(),
            font: "/unavailable/font".into(),
            font_sha256: "0".repeat(64),
            deadline: Duration::from_secs(60),
        },
    };
    let id = uuid::Uuid::new_v4().to_string();
    let mut state = State::queued();
    let mut lifecycle = state.lifecycle().unwrap();
    lifecycle.start().unwrap();
    state.update(lifecycle);
    sessions
        .store
        .create_job(&StoredJob {
            id: id.clone(),
            kind: JobKind::Render,
            session_id: snapshot.session_id,
            input: serde_json::to_vec(&input).unwrap(),
            version: 0,
            state: serde_json::to_vec(&state).unwrap(),
        })
        .unwrap();
    drop(sessions);
    let restored = Sessions::open(&db).unwrap();
    let job = restored.store.load_job(&id).unwrap().unwrap();
    let (_, state) = decode(&job).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert!(state.output.is_none());
    assert!(state.error.unwrap().message.contains("残存確認は未実施"));
    assert_eq!(
        fs::read(directory.join("partial.mp4")).unwrap(),
        b"unverified partial"
    );
    drop(restored);
    let reopened = Sessions::open(&db).unwrap();
    assert_eq!(reopened.store.load_job(&id).unwrap().unwrap(), job);
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_requires_success_and_preserves_job_and_project() {
    use sha2::{Digest, Sha256};
    use std::sync::{Arc, Mutex, atomic::Ordering};
    use sungeki_protocol::{ErrorCode, PublishRenderArgs, RenderJobRequest, RenderOutput};
    let root = std::env::temp_dir().join(format!("vvm-publish-host-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "保存APIのfixture".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let bytes = b"storage fixture, not actual media";
    fs::write(root.join("output.mp4"), bytes).unwrap();
    let input = Input {
        purpose: sungeki_protocol::RenderPurpose::Final,
        parent_job_id: None,
        source_revision: 0,
        content_hash: sungeki_storage::project_hash(&snapshot.project).unwrap(),
        project: snapshot.project.clone(),
        directory: root.clone(),
        runtime: sungeki_render::RenderRuntime {
            vrm: None,
            python: "/unused".into(),
            image_worker: "/unused".into(),
            ffmpeg: "/unused".into(),
            ffprobe: "/unused".into(),
            font: "/unused".into(),
            font_sha256: "0".repeat(64),
            deadline: Duration::from_secs(60),
        },
    };
    let job = StoredJob {
        id: uuid::Uuid::new_v4().to_string(),
        kind: JobKind::Render,
        session_id: snapshot.session_id.clone(),
        input: serde_json::to_vec(&input).unwrap(),
        version: 0,
        state: serde_json::to_vec(&State {
            chunk_sha256: vec![format!("{:x}", Sha256::digest(bytes))],
            phase: JobPhase::Succeeded,
            progress: None,
            error: None,
            output: Some(RenderOutput {
                path: root.join("output.mp4").to_str().unwrap().into(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                byte_length: bytes.len() as u64,
                frame_count: 1,
                font_sha256: "0".repeat(64),
                layout_version: sungeki_render::CURRENT_LAYOUT_VERSION,
            }),
        })
        .unwrap(),
    };
    sessions.store.create_job(&job).unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let read = |offset, length| {
        Sessions::execute(
            &shared,
            SessionRequest::RenderJob {
                request: RenderJobRequest::Read {
                    args: sungeki_protocol::ReadRenderArgs {
                        job_id: job.id.clone(),
                        offset,
                        length,
                    },
                },
            },
        )
    };
    let SessionResponse::RenderBytes { data } = read(1, 3).unwrap() else {
        panic!("bytes response")
    };
    use base64::Engine;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(data.base64)
            .unwrap(),
        &bytes[1..4]
    );
    assert_eq!(data.total, bytes.len() as u64);
    assert!(read(0, 0).is_err());
    assert!(read(0, 262145).is_err());
    assert!(read(bytes.len() as u64, 1).is_err());
    fs::write(root.join("output.mp4"), vec![b'x'; bytes.len()]).unwrap();
    assert_eq!(
        read(0, 1).unwrap_err().code,
        ErrorCode::ExternalModification
    );
    fs::write(root.join("output.mp4"), bytes).unwrap();
    let target = root.join("published.mp4");
    let publish = |id: &str, path: &std::path::Path| {
        Sessions::execute(
            &shared,
            SessionRequest::RenderJob {
                request: RenderJobRequest::Publish {
                    args: PublishRenderArgs {
                        job_id: id.into(),
                        path: path.to_str().unwrap().into(),
                    },
                },
            },
        )
    };
    let mut preview = job.clone();
    preview.id = uuid::Uuid::new_v4().to_string();
    let mut preview_input: Input = serde_json::from_slice(&preview.input).unwrap();
    preview_input.purpose = sungeki_protocol::RenderPurpose::EditingPreview;
    preview.input = serde_json::to_vec(&preview_input).unwrap();
    shared.lock().unwrap().store.create_job(&preview).unwrap();
    let rejected = root.join("preview-must-not-publish.mp4");
    assert_eq!(
        publish(&preview.id, &rejected).unwrap_err().code,
        ErrorCode::ValidationFailed
    );
    assert!(!rejected.exists());
    let mismatch = crate::render_api::execute(
        &shared,
        RenderJobRequest::Start {
            args: sungeki_protocol::StartRenderArgs {
                wait_ms: None,
                session_id: snapshot.session_id.clone(),
                job_id: preview.id.clone(),
                expected_revision: 0,
            },
        },
    )
    .unwrap_err();
    assert_eq!(mismatch.code, ErrorCode::Conflict);
    for repeat in [false, true] {
        let SessionResponse::RenderPublished { publication } = publish(&job.id, &target).unwrap()
        else {
            panic!()
        };
        assert_eq!(publication.already_present, repeat);
        assert!(!publication.stale);
        assert_eq!(publication.source_revision, 0);
        assert_eq!(publication.session_id, snapshot.session_id);
        assert_eq!(
            publication.output.path,
            target.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(fs::read(&target).unwrap(), bytes);
    }
    assert_eq!(
        publish("missing", &target).unwrap_err().code,
        ErrorCode::NotFound
    );
    let failed_id = uuid::Uuid::new_v4().to_string();
    let mut failed = job.clone();
    failed.id = failed_id.clone();
    failed.state = serde_json::to_vec(&State {
        chunk_sha256: Vec::new(),
        phase: JobPhase::Failed,
        progress: None,
        output: None,
        error: None,
    })
    .unwrap();
    shared.lock().unwrap().store.create_job(&failed).unwrap();
    assert_eq!(
        publish(&failed_id, &target).unwrap_err().code,
        ErrorCode::Conflict
    );
    {
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.store.load_job(&job.id).unwrap().unwrap(), job);
        let (_, current) = sessions.load(&snapshot.session_id).unwrap();
        assert_eq!(current.history().project(), &snapshot.project);
        assert_eq!(current.history().revision(), 0);
        assert_eq!(sessions.render_runtime.active.load(Ordering::SeqCst), 0);
    }
    // A later project edit does not alter the completed video, but must be disclosed.
    let edit = serde_json::json!({"type":"edit","session_id":snapshot.session_id,"edit":{
        "operation_id":uuid::Uuid::new_v4().to_string(),"expected_revision":0,
        "action":{"type":"apply","commands":[{"type":"rename_project","title":"変更後"}]}
    }});
    Sessions::execute(&shared, serde_json::from_value(edit).unwrap()).unwrap();
    let SessionResponse::RenderPublished { publication } = publish(&job.id, &target).unwrap()
    else {
        panic!()
    };
    assert!(publication.stale);
    fs::write(root.join("output.mp4"), b"changed source").unwrap();
    assert_eq!(
        publish(&job.id, &target).unwrap_err().code,
        ErrorCode::ExternalModification
    );
    assert_eq!(fs::read(&target).unwrap(), bytes);
    shared
        .lock()
        .unwrap()
        .render_runtime
        .stopping
        .store(true, Ordering::SeqCst);
    assert_eq!(
        publish(&job.id, &target).unwrap_err().code,
        ErrorCode::Cancelled
    );
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}
