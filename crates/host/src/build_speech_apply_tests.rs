use super::*;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_editor::{EditAction, EditRequest};

pub(super) async fn ready() -> (std::path::PathBuf, Arc<Mutex<Sessions>>, String, String) {
    let (root, mut sessions, parent, input, child) = super::speech_input_tests::setup();
    crate::speech_journal::create(
        &mut sessions,
        child.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    crate::speech_dispatch::Reservation::acquire(&mut sessions, &child)
        .unwrap()
        .begin(&mut sessions)
        .unwrap()
        .confirm_finished(&mut sessions)
        .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    crate::speech_output::complete(
        &shared,
        &child,
        crate::speech_output_tests::response(),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();
    (root, shared, parent.id, child)
}
#[tokio::test]
async fn apply_saved_original_with_ripple_and_both_receipts_then_replay_after_undo() {
    let (root, shared, parent, child) = ready().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    let receipt = crate::build_record::apply_speech(&shared, &parent, &child)
        .await
        .unwrap();
    {
        let sessions = shared.lock().unwrap();
        let (_, p) = decode(&sessions.store.load_job(&parent).unwrap().unwrap()).unwrap();
        let (_, c) =
            crate::speech_journal::decode(&sessions.store.load_job(&child).unwrap().unwrap())
                .unwrap();
        assert_eq!(p.receipts.len(), 3);
        assert!(p.child_id.is_none());
        assert_eq!(p.receipts[2].revision, receipt.revision);
        assert!(
            matches!(c.application,crate::speech_state::Application::Applied {revision,..} if revision==receipt.revision)
        );
        let (_, session) = sessions.load("session").unwrap();
        let project = session.history().project();
        assert_eq!(
            project.clips[0].duration_us.0,
            c.artifact.unwrap().duration_us
        );
        assert_eq!(
            project.clips[1].start_us.0,
            project.clips[0].duration_us.0 + 350_000
        );
    }
    assert_eq!(
        crate::build_record::apply_speech(&shared, &parent, &child)
            .await
            .unwrap(),
        receipt
    );
    {
        let mut sessions = shared.lock().unwrap();
        let (stored, mut candidate) = sessions.load("session").unwrap();
        candidate
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: stored.revision,
                action: EditAction::Undo {
                    expected_operation_id: receipt.operation_id.clone(),
                },
            })
            .unwrap();
        let prior = sungeki_editor::EditSession::restore(&before.snapshot).unwrap();
        assert_eq!(candidate.history().project(), prior.history().project());
        sessions
            .store
            .commit(
                Some(stored.version),
                &StoredSession {
                    id: stored.id,
                    revision: candidate.history().revision(),
                    version: stored.version + 1,
                    snapshot: candidate.snapshot().unwrap().into(),
                },
            )
            .unwrap();
    }
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    let undone = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::build_record::apply_speech(&shared, &parent, &child)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load("session")
            .unwrap()
            .unwrap(),
        undone
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn missing_original_cancellation_or_user_edit_preserves_all_records() {
    for mode in 0..3 {
        let (root, shared, parent, child) = ready().await;
        let (before, p, c) = {
            let mut sessions = shared.lock().unwrap();
            let mut p = sessions.store.load_job(&parent).unwrap().unwrap();
            let c = sessions.store.load_job(&child).unwrap().unwrap();
            if mode == 1 {
                p = crate::build_record::request_cancel(&mut sessions, &p).unwrap();
            } else if mode == 2 {
                let (stored, mut candidate) = sessions.load("session").unwrap();
                candidate
                    .execute(EditRequest {
                        operation_id: uuid::Uuid::new_v4().to_string(),
                        expected_revision: stored.revision,
                        action: EditAction::Apply {
                            commands: vec![sungeki_editor::Command::RenameProject {
                                title: "利用者の編集".into(),
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
                            revision: candidate.history().revision(),
                            version: stored.version + 1,
                            snapshot: candidate.snapshot().unwrap().into(),
                        },
                    )
                    .unwrap();
            } else {
                let (_, state) = crate::speech_journal::decode(&c).unwrap();
                std::fs::remove_file(state.artifact.unwrap().path).unwrap();
            }
            (sessions.store.load("session").unwrap().unwrap(), p, c)
        };
        assert!(
            crate::build_record::apply_speech(&shared, &parent, &child)
                .await
                .is_err()
        );
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
        assert_eq!(sessions.store.load_job(&parent).unwrap().unwrap(), p);
        assert_eq!(sessions.store.load_job(&child).unwrap().unwrap(), c);
        drop(sessions);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}
