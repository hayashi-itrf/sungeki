use sungeki_editor::*;
use sungeki_project::*;

fn id(n: u32) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn command(reading: &str) -> Command {
    Command::SetPronunciationDictionary {
        entries: vec![PronunciationEntry {
            written: "固有名詞".into(),
            spoken: reading.into(),
        }],
    }
}
fn request(n: u32, revision: u32, action: EditAction) -> EditRequest {
    EditRequest {
        operation_id: id(n),
        expected_revision: revision,
        action,
    }
}

#[test]
fn dictionary_is_atomic_undoable_and_restored_with_receipts() {
    let original = Project::empty(id(1), "辞書編集".into());
    let mut session = EditSession::new(original.clone()).unwrap();
    let edit = request(
        2,
        0,
        EditAction::Apply {
            commands: vec![command("こゆうめいし")],
        },
    );
    let receipt = session.execute(edit.clone()).unwrap();
    let mut restored = EditSession::restore(&session.snapshot().unwrap()).unwrap();
    assert_eq!(restored.execute(edit).unwrap(), receipt);
    assert_eq!(
        restored.history().project().pronunciation_dictionary[0].spoken,
        "こゆうめいし"
    );
    restored
        .execute(request(
            3,
            1,
            EditAction::Undo {
                expected_operation_id: id(2),
            },
        ))
        .unwrap();
    assert_eq!(restored.history().project(), &original);
    restored
        .execute(request(
            4,
            2,
            EditAction::Redo {
                expected_operation_id: id(2),
            },
        ))
        .unwrap();
    assert_eq!(
        restored.history().project().pronunciation_dictionary.len(),
        1
    );
    assert!(matches!(
        restored.execute(request(
            5,
            0,
            EditAction::Apply {
                commands: vec![command("古い読み")]
            }
        )),
        Err(OperationError::History(HistoryError::Conflict))
    ));
}

#[test]
fn invalid_dictionary_rolls_back_whole_batch_and_empty_dictionary_clears_it() {
    let original = Project::empty(id(1), "元の作品".into());
    let commands = [
        Command::RenameProject {
            title: "変わらない".into(),
        },
        command(" "),
    ];
    assert!(apply_batch(&original, &commands).is_err());
    assert_eq!(original.title, "元の作品");
    let changed = apply_batch(&original, &[command("こゆうめいし")]).unwrap();
    let cleared = apply_batch(
        &changed,
        &[Command::SetPronunciationDictionary { entries: vec![] }],
    )
    .unwrap();
    assert_eq!(cleared, original);
}
