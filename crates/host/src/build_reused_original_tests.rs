use crate::build_record::{
    continuation::Continuation, reused_original::publish, speech_reuse::verify,
};
use std::{os::unix::fs::MetadataExt, sync::atomic::AtomicBool};

#[tokio::test]
async fn carries_verified_bytes_and_lip_sync_to_independent_original_without_changing_journals() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let proofs = vec![(0, verify(&shared, &parent, &child, &target).await.unwrap())];
    let source = Continuation::from_verified(&proofs, 2).unwrap();
    let proof = &proofs[0].1;
    assert_eq!(
        source.source_for(0),
        Some((parent.as_str(), child.as_str()))
    );
    assert_eq!(source.source_for(1), None);
    let (directory, before) = {
        let sessions = shared.lock().unwrap();
        (
            sessions.speech_directory.as_ref().unwrap().clone(),
            sessions.store.load("session").unwrap().unwrap(),
        )
    };
    let original = std::fs::read(&proof.artifact.path).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let copied = publish(
        &directory,
        &id,
        0,
        target.input_hash(),
        &source,
        proof,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(std::fs::read(&copied.path).unwrap(), original);
    assert_eq!(copied.lip_sync, proof.artifact.lip_sync);
    assert_ne!(
        std::fs::metadata(&copied.path).unwrap().ino(),
        std::fs::metadata(&proof.artifact.path).unwrap().ino()
    );
    assert_eq!(std::fs::read(&proof.artifact.path).unwrap(), original);
    std::fs::remove_file(&proof.artifact.path).unwrap();
    // Publication consumes captured verified bytes, never reopens the source path.
    let second = publish(
        &directory,
        &uuid::Uuid::new_v4().to_string(),
        0,
        target.input_hash(),
        &source,
        proof,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(std::fs::read(&second.path).unwrap(), original);
    assert_eq!(
        crate::speech_original::load(&copied)
            .unwrap()
            .info()
            .content_hash,
        proof.artifact.sha256
    );
    let sessions = shared.lock().unwrap();
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    assert_eq!(
        sessions.store.load_job(&parent).unwrap().unwrap(),
        proof.parent
    );
    assert_eq!(
        sessions.store.load_job(&child).unwrap().unwrap(),
        proof.child
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn refuses_invalid_target_and_reconciles_identical_destination_without_overwriting() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let proofs = vec![(0, verify(&shared, &parent, &child, &target).await.unwrap())];
    let source = Continuation::from_verified(&proofs, 2).unwrap();
    let proof = &proofs[0].1;
    let directory = shared
        .lock()
        .unwrap()
        .speech_directory
        .as_ref()
        .unwrap()
        .clone();
    let id = uuid::Uuid::new_v4().to_string();
    for (index, hash, cancelled) in [
        (0, target.input_hash(), true),
        (1, target.input_hash(), false),
        (0, "different-input", false),
    ] {
        assert!(
            publish(
                &directory,
                &id,
                index,
                hash,
                &source,
                proof,
                &AtomicBool::new(cancelled)
            )
            .is_err()
        );
        assert!(!directory.join(&id).exists());
    }
    assert!(
        publish(
            &directory,
            &child,
            0,
            target.input_hash(),
            &source,
            proof,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let copied = publish(
        &directory,
        &id,
        0,
        target.input_hash(),
        &source,
        proof,
        &AtomicBool::new(false),
    )
    .unwrap();
    let before = std::fs::read(&copied.path).unwrap();
    let replay = publish(
        &directory,
        &id,
        0,
        target.input_hash(),
        &source,
        proof,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(replay.path, copied.path);
    assert_eq!(std::fs::read(&copied.path).unwrap(), before);
    assert_eq!(std::fs::read(&proof.artifact.path).unwrap(), before);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
