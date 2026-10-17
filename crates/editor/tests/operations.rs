use sungeki_editor::*;
use sungeki_project::Project;
fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn rename(op: u32, revision: u32, title: &str) -> EditRequest {
    EditRequest {
        operation_id: id(op),
        expected_revision: revision,
        action: EditAction::Apply {
            commands: vec![Command::RenameProject {
                title: title.into(),
            }],
        },
    }
}
#[test]
fn retry_survives_restore_and_payload_reuse_is_rejected() {
    let mut session = EditSession::new(Project::empty(id(1), "before".into())).unwrap();
    let request = rename(10, 0, "after");
    let receipt = session.execute(request.clone()).unwrap();
    let mut restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(restored.execute(request).unwrap(), receipt);
    assert_eq!(restored.history().revision(), 1);
    assert!(matches!(
        restored.execute(rename(10, 0, "different")),
        Err(OperationError::ReusedId)
    ));
    assert_eq!(restored.history().project().title, "after");
}
#[test]
fn undo_checks_operation_and_is_itself_idempotent() {
    let mut session = EditSession::new(Project::empty(id(1), "before".into())).unwrap();
    session.execute(rename(10, 0, "after")).unwrap();
    let mut undo = EditRequest {
        operation_id: id(11),
        expected_revision: 1,
        action: EditAction::Undo {
            expected_operation_id: id(99),
        },
    };
    assert!(matches!(
        session.execute(undo.clone()),
        Err(OperationError::WrongUndoTarget)
    ));
    undo.action = EditAction::Undo {
        expected_operation_id: id(10),
    };
    let receipt = session.execute(undo.clone()).unwrap();
    assert_eq!(session.execute(undo).unwrap(), receipt);
    assert_eq!(session.history().project().title, "before");
    session
        .execute(EditRequest {
            operation_id: id(12),
            expected_revision: 2,
            action: EditAction::Redo {
                expected_operation_id: id(10),
            },
        })
        .unwrap();
    assert_eq!(session.history().project().title, "after");
    assert_eq!(session.history().undo_operation_id(), Some(id(10).as_str()));
}
#[test]
fn receipts_outlive_undo_limit_including_noop() {
    let mut session = EditSession::new(Project::empty(id(1), "0".into())).unwrap();
    let first = rename(10, 0, "0");
    let receipt = session.execute(first.clone()).unwrap();
    assert!(receipt.no_op);
    for revision in 0..105 {
        session
            .execute(rename(revision + 20, revision, &(revision + 1).to_string()))
            .unwrap();
    }
    let mut restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(restored.execute(first).unwrap(), receipt);
    assert_eq!(restored.history().revision(), 105);
}
