use super::*;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use sungeki_protocol::RenderOutput;

// A byte fixture, deliberately not evidence of successful media encoding.
pub(super) async fn ready() -> (
    std::path::PathBuf,
    Arc<Mutex<Sessions>>,
    StoredJob,
    StoredJob,
) {
    let (root, shared, parent, child) = super::build_render_tests::ready().await;
    let (input, _) = crate::render_record::decode(&child).unwrap();
    std::fs::create_dir(&input.directory).unwrap();
    let path = input.directory.join("output.mp4");
    let bytes = b"completed-render-byte-fixture";
    std::fs::write(&path, bytes).unwrap();
    let output = RenderOutput {
        path: path.to_string_lossy().into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        byte_length: bytes.len() as u64,
        frame_count: decode(&parent).unwrap().1.preflight.unwrap().frame_count,
        font_sha256: input.runtime.font_sha256,
        layout_version: 8,
    };
    let state = crate::render_record::State {
        chunk_sha256: Vec::new(),
        phase: JobPhase::Succeeded,
        progress: None,
        output: Some(output),
        error: None,
    };
    let child = {
        let mut sessions = shared.lock().unwrap();
        crate::render_record::save(&mut sessions, &child, &state).unwrap();
        sessions.store.load_job(&child.id).unwrap().unwrap()
    };
    (root, shared, parent, child)
}

#[tokio::test]
async fn render_receipt_rechecks_bytes_and_replays_without_advancing_twice() {
    let (root, shared, parent, child) = ready().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    let output = crate::build_record::accept_render(&shared, &parent).unwrap();
    let accepted = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    let (input, state) = decode(&accepted).unwrap();
    assert!(state.child_id.is_none());
    assert_eq!(state.receipts.len(), usize::from(input.speech_count) + 7);
    assert_eq!(state.rendered.unwrap().sha256, output.sha256);
    for _ in 0..2 {
        assert_eq!(
            crate::build_record::accept_render(&shared, &accepted)
                .unwrap()
                .sha256,
            output.sha256
        );
    }
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    {
        let sessions = shared.lock().unwrap();
        assert_eq!(
            sessions.store.load_job(&parent.id).unwrap().unwrap(),
            accepted
        );
        assert_eq!(sessions.store.load_job(&child.id).unwrap().unwrap(), child);
        assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    }
    // Completed receipt survives restart; bytes are still required on replay.
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    let restored = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    assert_eq!(decode(&restored).unwrap().1.phase, JobPhase::Interrupted);
    crate::build_record::accept_render(&shared, &restored).unwrap();
    std::fs::remove_file(&output.path).unwrap();
    assert!(crate::build_record::accept_render(&shared, &restored).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        restored
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn changed_missing_or_symlink_output_never_advances_parent() {
    let (root, shared, parent, child) = ready().await;
    let (_, result) = crate::render_record::decode(&child).unwrap();
    let output = result.output.unwrap();
    let original = std::fs::read(&output.path).unwrap();
    std::fs::write(&output.path, vec![b'x'; original.len()]).unwrap();
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    std::fs::remove_file(&output.path).unwrap();
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    let target = root.join("same-bytes.mp4");
    std::fs::write(&target, original).unwrap();
    std::os::unix::fs::symlink(&target, &output.path).unwrap();
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        parent
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn foreign_output_and_cancelled_parent_cannot_complete_render_step() {
    let (root, shared, parent, child) = ready().await;
    let (_, mut state) = crate::render_record::decode(&child).unwrap();
    state.output.as_mut().unwrap().frame_count += 1;
    {
        let mut sessions = shared.lock().unwrap();
        crate::render_record::save(&mut sessions, &child, &state).unwrap();
    }
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    state.output.as_mut().unwrap().frame_count -= 1;
    let cancelling = {
        let mut sessions = shared.lock().unwrap();
        let current = sessions.store.load_job(&child.id).unwrap().unwrap();
        crate::render_record::save(&mut sessions, &current, &state).unwrap();
        crate::build_record::request_cancel(&mut sessions, &parent).unwrap()
    };
    assert!(crate::build_record::accept_render(&shared, &cancelling).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        cancelling
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn edited_project_or_saved_document_cannot_accept_previous_video() {
    use sungeki_editor::{Command, EditAction, EditRequest};
    let (root, shared, parent, _) = ready().await;
    let (input, _) = decode(&parent).unwrap();
    let document = std::fs::read(&input.document_path).unwrap();
    std::fs::write(&input.document_path, b"changed").unwrap();
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    std::fs::write(&input.document_path, document).unwrap();
    {
        let mut sessions = shared.lock().unwrap();
        let (stored, mut current) = sessions.load("session").unwrap();
        current
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: stored.revision,
                action: EditAction::Apply {
                    commands: vec![Command::RenameProject {
                        title: "編集中の作品".into(),
                    }],
                },
            })
            .unwrap();
        sessions
            .store
            .commit(
                Some(stored.version),
                &StoredSession {
                    id: stored.id,
                    revision: stored.revision + 1,
                    version: stored.version + 1,
                    snapshot: current.snapshot().unwrap().into(),
                },
            )
            .unwrap();
    }
    assert!(crate::build_record::accept_render(&shared, &parent).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        parent
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
