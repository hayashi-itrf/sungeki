use super::*;
use crate::build_record::{Reservation, reserve};
use crate::build_state::BakeOutcome;
use sungeki_editor::{Command, EditAction, EditRequest};

pub(super) async fn ready() -> (
    std::path::PathBuf,
    std::sync::Arc<std::sync::Mutex<Sessions>>,
    StoredJob,
) {
    let (root, shared, row) = super::preflight_tests::ready().await;
    crate::build_record::validate_output(&shared, &row)
        .await
        .unwrap();
    let row = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&row.id).unwrap().unwrap();
        let Reservation::New(row) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        row
    };
    (root, shared, row)
}
#[tokio::test]
async fn records_no_bake_without_changing_originals_project_or_revision() {
    let (root, shared, row) = ready().await;
    {
        let mut sessions = shared.lock().unwrap();
        let before = sessions.store.load("session").unwrap().unwrap();
        let original = std::fs::read(root.join("source.png")).unwrap();
        assert_eq!(
            crate::build_record::finish_bake(&mut sessions, &row).unwrap(),
            BakeOutcome::NotRequired
        );
        let done = sessions.store.load_job(&row.id).unwrap().unwrap();
        let (_, state) = decode(&done).unwrap();
        assert_eq!(state.receipts.len(), 7);
        assert_eq!(state.bake, Some(BakeOutcome::NotRequired));
        assert!(state.child_id.is_none());
        assert_eq!(state.expected_revision, before.revision);
        assert_eq!(state.receipts[5].revision, state.receipts[6].revision);
        assert_eq!(
            crate::build_record::finish_bake(&mut sessions, &done).unwrap(),
            BakeOutcome::NotRequired
        );
        assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), done);
        assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
        assert_eq!(std::fs::read(root.join("source.png")).unwrap(), original);
    }
    drop(shared);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let done = sessions.store.load_job(&row.id).unwrap().unwrap();
    assert_eq!(decode(&done).unwrap().1.phase, JobPhase::Interrupted);
    assert_eq!(
        crate::build_record::finish_bake(&mut sessions, &done).unwrap(),
        BakeOutcome::NotRequired
    );
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn refuses_cancel_or_edited_project_and_rejects_missing_outcome_in_record() {
    for cancel in [false, true] {
        let (root, shared, row) = ready().await;
        {
            let mut sessions = shared.lock().unwrap();
            if cancel {
                crate::build_record::request_cancel(&mut sessions, &row).unwrap();
            } else {
                let (stored, mut candidate) = sessions.load("session").unwrap();
                candidate
                    .execute(EditRequest {
                        operation_id: uuid::Uuid::new_v4().to_string(),
                        expected_revision: stored.revision,
                        action: EditAction::Apply {
                            commands: vec![Command::RenameProject {
                                title: "編集済み".into(),
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
                            version: stored.version + 1,
                            revision: candidate.history().revision(),
                            snapshot: candidate.snapshot().unwrap().into(),
                        },
                    )
                    .unwrap();
            }
            let before = sessions.store.load("session").unwrap().unwrap();
            let current = sessions.store.load_job(&row.id).unwrap().unwrap();
            assert!(crate::build_record::finish_bake(&mut sessions, &current).is_err());
            assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
            assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), current);
        }
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
    let input = input();
    let mut state = state(&input, JobPhase::Succeeded);
    state.bake = None;
    assert!(state.validate(&input).is_err());
    state.bake = Some(BakeOutcome::NotRequired);
    state.receipts[6].revision += 1;
    assert!(state.validate(&input).is_err());
}
