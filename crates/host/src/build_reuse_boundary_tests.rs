use super::*;
use crate::build_record as build;
use std::sync::{Arc, Mutex};
use sungeki_protocol::{BuildVideoRequest, SessionResponse};

fn progress(shared: &Arc<Mutex<Sessions>>, id: &str) -> String {
    let SessionResponse::BuildVideoJob { job } = build::execute(
        shared,
        BuildVideoRequest::Get {
            job_id: id.into(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.progress.as_ref().unwrap().fraction, None);
    job.progress.unwrap().stage
}

#[tokio::test]
async fn reuse_progress_distinguishes_inspection_from_application_without_writes() {
    let (root, shared, parent, child, target, proof, artifact) =
        super::reused_child_tests::setup().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        progress(&shared, &parent.id),
        "保存済み音声を検査・引き継ぎ中（1 / 2）"
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        before
    );
    let registered =
        build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
            .await
            .unwrap();
    assert_eq!(
        progress(&shared, &parent.id),
        "引き継いだ音声を作品へ反映中（1 / 2）"
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&child)
            .unwrap()
            .unwrap(),
        registered
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        before
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn restart_after_publication_does_not_invent_success_or_overwrite_an_orphan() {
    let (root, shared, parent, child, target, proof, artifact) =
        super::reused_child_tests::setup().await;
    let original = std::fs::read(&artifact.path).unwrap();
    let before = shared
        .lock()
        .unwrap()
        .store
        .load(&parent.session_id)
        .unwrap()
        .unwrap();
    // Deliberately stop between filesystem publication and child registration.
    // This models durable restart state; it is not an OS hard-kill test.
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    let recovered = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    let (input, state) = build::decode(&recovered).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.child_id.as_deref(), Some(child.as_str()));
    assert_eq!(state.receipts.len(), 2);
    assert!(
        build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
            .await
            .is_err()
    );
    let sessions = shared.lock().unwrap();
    assert!(sessions.store.load_job(&child).unwrap().is_none());
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        before
    );
    assert!(!build::settlement::ready(&sessions, &recovered).unwrap());
    let found = build::reused_original::publish(
        sessions.speech_directory.as_ref().unwrap(),
        &child,
        0,
        target.input_hash(),
        input.continuation.as_ref().unwrap(),
        &proof,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(found.path, artifact.path);
    assert_eq!(std::fs::read(&artifact.path).unwrap(), original);
    assert_eq!(
        sessions.store.load_job(&proof.child.id).unwrap().unwrap(),
        proof.child
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
