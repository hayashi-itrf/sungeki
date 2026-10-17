use super::*;
use crate::build_record as build;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn interrupted_original_is_registered_and_applied_without_restarting_parent() {
    for registered_before_restart in [false, true] {
        let (root, shared, parent, child, target, proof, artifact) =
            super::reused_child_tests::setup().await;
        let original = std::fs::read(&artifact.path).unwrap();
        if registered_before_restart {
            build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
                .await
                .unwrap();
        }
        let before = shared
            .lock()
            .unwrap()
            .store
            .load(&parent.session_id)
            .unwrap()
            .unwrap();
        drop(shared);
        let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
        let interrupted = shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            build::decode(&interrupted).unwrap().1.phase,
            JobPhase::Interrupted
        );
        assert!(
            !build::settlement::ready(&shared.lock().unwrap(), &interrupted).unwrap()
                || registered_before_restart
        );
        shared.lock().unwrap().speech_directory = Err(sungeki_protocol::ApiError::new(
            sungeki_protocol::ErrorCode::CapabilityUnavailable,
            "起動直後の未確認状態",
        ));
        let first = public_reconcile(&shared, &parent.id, &child).await.unwrap();
        let replay = public_reconcile(&shared, &parent.id, &child).await.unwrap();
        assert_eq!(
            serde_json::to_value(first).unwrap(),
            serde_json::to_value(replay).unwrap()
        );
        let receipt = build::apply_speech(&shared, &parent.id, &child)
            .await
            .unwrap();
        let applied = {
            let sessions = shared.lock().unwrap();
            let parent_after = sessions.store.load_job(&parent.id).unwrap().unwrap();
            let state = build::decode(&parent_after).unwrap().1;
            assert_eq!(state.phase, JobPhase::Interrupted);
            assert!(state.child_id.is_none());
            assert_eq!(state.receipts.len(), 3);
            assert_eq!(state.receipts[2].revision, receipt.revision);
            assert!(build::settlement::ready(&sessions, &parent_after).unwrap());
            let applied = sessions.store.load(&parent.session_id).unwrap().unwrap();
            assert_eq!(applied.revision, before.revision + 1);
            assert_eq!(
                sessions.store.load_job(&proof.child.id).unwrap().unwrap(),
                proof.child
            );
            assert_eq!(
                sessions.store.load_job(&proof.parent.id).unwrap().unwrap(),
                proof.parent
            );
            let row = sessions.store.load_job(&child).unwrap().unwrap();
            let state = crate::speech_journal::decode_input(&row).unwrap().1;
            assert_eq!(state.execution, crate::speech_state::Execution::Reused);
            assert!(matches!(
                state.application,
                crate::speech_state::Application::Applied { .. }
            ));
            applied
        };
        assert!(
            build::reuse_reconcile::run(&shared, &interrupted)
                .await
                .is_err()
        );
        drop(shared);
        let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
        assert_eq!(
            build::apply_speech(&shared, &parent.id, &child)
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(
            shared
                .lock()
                .unwrap()
                .store
                .load(&parent.session_id)
                .unwrap()
                .unwrap(),
            applied
        );
        // The reconciled audio is a valid source for a new continuation.
        let new_proof = build::speech_reuse::verify(&shared, &parent.id, &child, &target)
            .await
            .unwrap();
        assert_eq!(new_proof.artifact.sha256, artifact.sha256);
        assert_eq!(std::fs::read(&artifact.path).unwrap(), original);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn reconciliation_rejects_running_or_missing_or_modified_original_without_writes() {
    for case in [
        "running", "missing", "modified", "edited", "busy", "stopping",
    ] {
        let (root, shared, parent, child, _, _, artifact) =
            super::reused_child_tests::setup().await;
        let shared = if case == "running" {
            shared
        } else {
            drop(shared);
            Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()))
        };
        if case == "missing" {
            std::fs::remove_file(&artifact.path).unwrap();
        }
        if case == "modified" {
            let mut bytes = std::fs::read(&artifact.path).unwrap();
            *bytes.last_mut().unwrap() ^= 1;
            std::fs::write(&artifact.path, bytes).unwrap();
        }
        if case == "edited" {
            let mut sessions = shared.lock().unwrap();
            let (stored, mut candidate) = sessions.load(&parent.session_id).unwrap();
            candidate
                .execute(sungeki_editor::EditRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    expected_revision: stored.revision,
                    action: sungeki_editor::EditAction::Apply {
                        commands: vec![sungeki_editor::Command::RenameProject {
                            title: "復旧前の編集".into(),
                        }],
                    },
                })
                .unwrap();
            sessions
                .store
                .commit(
                    Some(stored.version),
                    &sungeki_storage::StoredSession {
                        id: stored.id,
                        revision: candidate.history().revision(),
                        version: stored.version + 1,
                        snapshot: candidate.snapshot().unwrap().into(),
                    },
                )
                .unwrap();
        }
        if case == "stopping" {
            shared
                .lock()
                .unwrap()
                .build_runtime
                .stopping
                .store(true, std::sync::atomic::Ordering::Release);
        }
        if case == "busy" {
            shared
                .lock()
                .unwrap()
                .render_runtime
                .flags
                .lock()
                .unwrap()
                .insert(
                    child.clone(),
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                );
        }
        let (row, project) = {
            let s = shared.lock().unwrap();
            (
                s.store.load_job(&parent.id).unwrap().unwrap(),
                s.store.load(&parent.session_id).unwrap().unwrap(),
            )
        };
        assert!(build::reuse_reconcile::run(&shared, &row).await.is_err());
        let s = shared.lock().unwrap();
        assert_eq!(s.store.load_job(&parent.id).unwrap().unwrap(), row);
        assert_eq!(s.store.load(&parent.session_id).unwrap().unwrap(), project);
        assert!(s.store.load_job(&child).unwrap().is_none());
        drop(s);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}

async fn public_reconcile(
    shared: &Arc<Mutex<Sessions>>,
    parent: &str,
    child: &str,
) -> Result<sungeki_protocol::SessionResponse, sungeki_protocol::ApiError> {
    let shared = shared.clone();
    let args = sungeki_protocol::ReconcileBuildAudioArgs {
        job_id: parent.into(),
        child_job_id: child.into(),
    };
    tokio::task::spawn_blocking(move || {
        Sessions::execute(
            &shared,
            sungeki_protocol::SessionRequest::BuildVideo {
                request: sungeki_protocol::BuildVideoRequest::ReconcileAudio { args },
            },
        )
    })
    .await
    .unwrap()
}
