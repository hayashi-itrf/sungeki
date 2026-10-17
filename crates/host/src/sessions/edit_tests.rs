use super::*;
use sungeki_editor::{Command, EditAction};

fn snapshot(response: SessionResponse) -> SessionSnapshot {
    match response {
        SessionResponse::Snapshot { snapshot } | SessionResponse::Edited { snapshot, .. } => {
            snapshot
        }
        other => panic!("unexpected response: {other:?}"),
    }
}
fn rename(revision: u32, title: &str) -> EditRequest {
    EditRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: revision,
        action: EditAction::Apply {
            commands: vec![Command::RenameProject {
                title: title.into(),
            }],
        },
    }
}
fn setup() -> (std::path::PathBuf, Sessions, SessionSnapshot) {
    let root = std::env::temp_dir().join(format!("vvm-edit-response-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("recovery.sqlite")).unwrap();
    let initial = snapshot(
        sessions
            .request(SessionRequest::Create {
                title: "before".into(),
            })
            .unwrap(),
    );
    (root, sessions, initial)
}
#[test]
fn reused_response_matches_durable_state_and_post_commit_cursor() {
    let (root, mut sessions, initial) = setup();
    let id = initial.session_id.clone();
    let edit = rename(0, "after");
    let edited = snapshot(sessions.edit_session(id.clone(), edit.clone()).unwrap());
    assert_eq!(edited.project.title, "after");
    assert_eq!(edited.revision, 1);
    assert_eq!(edited.undo_operation_id.as_ref(), Some(&edit.operation_id));
    assert!(edited.dirty);
    assert_eq!(
        edited.event_cursor.event_seq,
        initial.event_cursor.event_seq + 1
    );
    let cursor = edited.event_cursor.clone();
    let replay = snapshot(sessions.edit_session(id.clone(), edit.clone()).unwrap());
    assert_eq!(
        serde_json::to_value(&edited).unwrap(),
        serde_json::to_value(&replay).unwrap()
    );
    let undone = snapshot(
        sessions
            .edit_session(
                id.clone(),
                EditRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    expected_revision: 1,
                    action: EditAction::Undo {
                        expected_operation_id: edit.operation_id,
                    },
                },
            )
            .unwrap(),
    );
    assert_eq!(undone.project.title, "before");
    assert_eq!(undone.revision, 2);
    assert_eq!(undone.event_cursor.event_seq, cursor.event_seq + 1);
    let (_, durable) = sessions.load(&id).unwrap();
    assert_eq!(&undone.project, durable.history().project());
    drop(sessions);
    let reopened = Sessions::open(&root.join("recovery.sqlite")).unwrap();
    let (_, durable) = reopened.load(&id).unwrap();
    assert_eq!(&undone.project, durable.history().project());
    assert_eq!(durable.history().revision(), 2);
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_commit_does_not_publish_candidate_or_advance_cursor() {
    let (root, mut sessions, initial) = setup();
    let id = initial.session_id.clone();
    // Warm the parsed cache, then fail an actual SQLite write after preflight.
    let committed = snapshot(
        sessions
            .edit_session(id.clone(), rename(0, "committed"))
            .unwrap(),
    );
    let db = rusqlite::Connection::open(root.join("recovery.sqlite")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_edit BEFORE UPDATE ON sessions BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    let request = rename(1, "must not escape");
    assert!(sessions.edit_session(id.clone(), request.clone()).is_err());
    let current = snapshot(
        sessions
            .request(SessionRequest::Get {
                session_id: id.clone(),
            })
            .unwrap(),
    );
    assert_eq!(
        serde_json::to_value(&current).unwrap(),
        serde_json::to_value(&committed).unwrap()
    );
    db.execute_batch("DROP TRIGGER reject_edit;").unwrap();
    let retried = snapshot(sessions.edit_session(id, request).unwrap());
    assert_eq!(retried.project.title, "must not escape");
    assert_eq!(
        retried.event_cursor.event_seq,
        committed.event_cursor.event_seq + 1
    );
    drop(db);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
