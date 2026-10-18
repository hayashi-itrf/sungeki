use crate::{
    frame_preview_record::{Input, State, decode},
    sessions::Sessions,
};
use std::{fs, time::Duration};
use sungeki_project::JobPhase;
use sungeki_protocol::{SessionRequest, SessionResponse};
use sungeki_storage::{JobKind, StoredJob};

#[test]
fn interrupted_frame_preview_keeps_its_work_and_never_claims_a_completed_output() {
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
        frame_index: 0,
        source_revision: 0,
        content_hash: sungeki_storage::project_hash(&snapshot.project).unwrap(),
        project: snapshot.project,
        directory: directory.clone(),
        runtime: sungeki_render::RenderRuntime {
            vrm: None,

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
            kind: JobKind::FramePreview,
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
fn output_version_marks_history_and_publication_stale_without_changing_project() {
    use sha2::{Digest, Sha256};
    use std::sync::{Arc, Mutex};
    let root = std::env::temp_dir().join(format!("vvm-output-version-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "描画版の検証".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let shared = Arc::new(Mutex::new(sessions));
    for (kind, request_type, filename) in [
        (JobKind::FramePreview, "frame_preview", "frame-0.png"),
        (JobKind::Render, "render_job", "output.mp4"),
    ] {
        // Storage fixture only: this does not claim a real rendering test.
        let bytes = b"output version storage fixture";
        fs::write(root.join(filename), bytes).unwrap();
        for version in [
            None,
            Some(7),
            Some(8),
            Some(sungeki_render::CURRENT_LAYOUT_VERSION),
        ] {
            let mut input = serde_json::json!({
                "source_revision":0,"content_hash":sungeki_storage::project_hash(&snapshot.project).unwrap(),
                "project":snapshot.project,"directory":root,
                "runtime":{"vrm":null,"python":"/unused","image_worker":"/unused",
                    "ffmpeg":"/unused","ffprobe":"/unused","font":"/unused",
                    "font_sha256":"0".repeat(64),"deadline":{"secs":60,"nanos":0}}
            });
            let mut output = serde_json::json!({"path":root.join(filename),
                "sha256":format!("{:x}",Sha256::digest(bytes)),"byte_length":bytes.len(),
                "font_sha256":"0".repeat(64),"layout_version":version});
            let mut state = serde_json::json!({"phase":if version.is_some(){"succeeded"}else{"failed"},
                "progress":null,"error":null});
            if kind == JobKind::FramePreview {
                input["frame_index"] = 0.into();
                output["frame_index"] = 0.into();
                output["time_us"] = 0.into();
                output["width"] = snapshot.project.canvas.width.into();
                output["height"] = snapshot.project.canvas.height.into();
            } else {
                input["purpose"] = "final".into();
                input["parent_job_id"] = serde_json::Value::Null;
                output["frame_count"] = 1.into();
                state["chunk_sha256"] = if version.is_some() {
                    serde_json::json!([format!("{:x}", Sha256::digest(bytes))])
                } else {
                    serde_json::json!([])
                };
            }
            state["output"] = if version.is_some() {
                output
            } else {
                serde_json::Value::Null
            };
            let stored = StoredJob {
                id: uuid::Uuid::new_v4().to_string(),
                kind,
                session_id: snapshot.session_id.clone(),
                version: 0,
                input: serde_json::to_vec(&input).unwrap(),
                state: serde_json::to_vec(&state).unwrap(),
            };
            shared.lock().unwrap().store.create_job(&stored).unwrap();
            let mut get = serde_json::json!({"type":request_type,"request":{"type":"get","job_id":stored.id}});
            if kind == JobKind::Render {
                get["request"]["wait_ms"] = 0.into();
            }
            let result = Sessions::execute(&shared, serde_json::from_value(get).unwrap()).unwrap();
            let result = serde_json::to_value(result).unwrap();
            let stale = version.is_some_and(|v| v != sungeki_render::CURRENT_LAYOUT_VERSION);
            assert_eq!(result["job"]["stale"], stale, "{kind:?}/{version:?}");
            if version.is_some() {
                let target = root.join(format!("{}-{filename}", stored.id));
                let publish = serde_json::json!({"type":request_type,"request":{"type":"publish",
                    "args":{"job_id":stored.id,"path":target}}});
                let result =
                    Sessions::execute(&shared, serde_json::from_value(publish).unwrap()).unwrap();
                let result = serde_json::to_value(result).unwrap();
                assert_eq!(result["publication"]["stale"], stale);
                assert_eq!(fs::read(target).unwrap(), bytes);
            }
            assert_eq!(
                shared
                    .lock()
                    .unwrap()
                    .store
                    .load_job(&stored.id)
                    .unwrap()
                    .unwrap(),
                stored
            );
        }
    }
    let sessions = shared.lock().unwrap();
    let (_, current) = sessions.load(&snapshot.session_id).unwrap();
    assert_eq!(current.history().revision(), 0);
    assert_eq!(current.history().project(), &snapshot.project);
    drop(sessions);
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}
