use super::*;
use crate::build_record::{self as build, reused_recovery::inspect};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    sync::{Arc, Mutex, atomic::AtomicBool},
};

#[tokio::test]
async fn reconciles_a_published_original_after_restart_without_writing_or_applying() {
    let (root, shared, parent, child, target, proof, artifact) =
        super::reused_child_tests::setup().await;
    let project = shared
        .lock()
        .unwrap()
        .store
        .load(&parent.session_id)
        .unwrap()
        .unwrap();
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    let sessions = shared.lock().unwrap();
    let row = sessions.store.load_job(&parent.id).unwrap().unwrap();
    let (input, state) = build::decode(&row).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    let directory = sessions.speech_directory.as_ref().unwrap();
    let before = std::fs::metadata(&artifact.path).unwrap();
    for _ in 0..2 {
        let found = inspect(
            directory,
            &child,
            0,
            target.input_hash(),
            input.continuation.as_ref().unwrap(),
            &proof,
            &AtomicBool::new(false),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            serde_json::to_value(found).unwrap(),
            serde_json::to_value(&artifact).unwrap()
        );
    }
    let after = std::fs::metadata(&artifact.path).unwrap();
    assert_eq!(
        (before.ino(), before.mtime(), before.mtime_nsec()),
        (after.ino(), after.mtime(), after.mtime_nsec())
    );
    assert_eq!(sessions.store.load_job(&parent.id).unwrap().unwrap(), row);
    assert!(sessions.store.load_job(&child).unwrap().is_none());
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        project
    );
    assert_eq!(
        sessions.store.load_job(&proof.child.id).unwrap().unwrap(),
        proof.child
    );
    assert!(
        inspect(
            directory,
            &uuid::Uuid::new_v4().to_string(),
            0,
            target.input_hash(),
            input.continuation.as_ref().unwrap(),
            &proof,
            &AtomicBool::new(false)
        )
        .unwrap()
        .is_none()
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rejects_partial_changed_symlinked_cancelled_and_unbound_candidates_without_repairing() {
    let (root, shared, parent, child, target, proof, artifact) =
        super::reused_child_tests::setup().await;
    let sessions = shared.lock().unwrap();
    let (input, _) = build::decode(&parent).unwrap();
    let source = input.continuation.as_ref().unwrap();
    let directory = sessions.speech_directory.as_ref().unwrap();
    let check = |id: &str, index, hash: &str, cancel| {
        inspect(
            directory,
            id,
            index,
            hash,
            source,
            &proof,
            &AtomicBool::new(cancel),
        )
    };
    assert!(check(&child, 0, target.input_hash(), true).is_err());
    assert!(check(&child, 1, target.input_hash(), false).is_err());
    assert!(check(&child, 0, "changed", false).is_err());
    assert!(check("../escape", 0, target.input_hash(), false).is_err());
    assert!(check(&proof.child.id, 0, target.input_hash(), false).is_err());
    let path = std::path::Path::new(&artifact.path);
    let bytes = std::fs::read(path).unwrap();
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(path, &changed).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    assert_eq!(std::fs::read(path).unwrap(), changed);
    std::fs::write(path, &bytes[..bytes.len() / 2]).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    std::fs::remove_file(path).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    symlink(&proof.artifact.path, path).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::hard_link(&proof.artifact.path, path).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::write(path, &bytes).unwrap();
    let child_dir = directory.join(&child);
    std::fs::set_permissions(&child_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    std::fs::set_permissions(&child_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let moved = directory.join("retained-original");
    std::fs::rename(&child_dir, &moved).unwrap();
    symlink(&moved, &child_dir).unwrap();
    assert!(check(&child, 0, target.input_hash(), false).is_err());
    assert_eq!(std::fs::read(moved.join("audio.wav")).unwrap(), bytes);
    assert!(sessions.store.load_job(&child).unwrap().is_none());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
