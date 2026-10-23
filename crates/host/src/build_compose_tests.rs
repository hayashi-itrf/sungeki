use super::*;
use sungeki_editor::{EditAction, EditRequest, EditSession};
use sungeki_project::Project;

#[test]
fn fixed_plan_ids_are_canonical_unique_and_match_recipe_shape() {
    let input = input();
    for mode in 0..7 {
        let mut changed = input.clone();
        match mode {
            0 => changed.plan.script.track_id = changed.recipe.assets[0].id.clone(),
            1 => changed.plan.speech_asset_ids[0] = changed.step_ids[0].clone(),
            2 => {
                changed.plan.layout.scenes[0].standing.pop();
            }
            3 => changed.plan.layout.foreword = None,
            4 => changed.step_ids[0] = uuid::Uuid::new_v4().simple().to_string(),
            5 => changed.plan.speech_operation_ids[0] = changed.plan.script.speech_ids[0].clone(),
            _ => {
                changed.plan.layout.opening = Some(sungeki_editor::BuildOpeningIds {
                    logo: None,
                    audio: vec![],
                    performers: vec![],
                })
            }
        }
        assert!(
            state(&changed, JobPhase::Running)
                .validate(&changed)
                .is_err()
        );
    }
}

#[test]
fn compose_commits_parent_and_project_together_and_replay_survives_restart_and_undo() {
    let root = std::env::temp_dir().join(format!("vvm-compose-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("db");
    let mut sessions = Sessions::open(&path).unwrap();
    let input = input();
    let original = Project::empty(input.plan.project_id.clone(), "新規".into());
    let session = EditSession::new(original.clone()).unwrap();
    sessions
        .store
        .commit(
            None,
            &StoredSession {
                id: "session".into(),
                revision: 0,
                version: 0,
                snapshot: session.snapshot().unwrap().into(),
            },
        )
        .unwrap();
    let mut row = job(&input, JobPhase::Running);
    let mut ready = state(&input, JobPhase::Running);
    ready.receipts.truncate(1);
    ready.expected_revision = 0;
    ready.child_id = Some(input.step_ids[1].clone());
    row.state = serde_json::to_vec(&ready).unwrap();
    sessions.store.create_job(&row).unwrap();
    let before = sessions.store.load("session").unwrap().unwrap();
    for interrupted in [false, true] {
        let mut blocked = ready.clone();
        if interrupted {
            blocked.phase = JobPhase::Interrupted;
            blocked.error = Some(invalid("中断"));
        } else {
            blocked.child_id = None;
        }
        let mut blocked_row = row.clone();
        blocked_row.state = serde_json::to_vec(&blocked).unwrap();
        assert!(crate::build_record::compose(&mut sessions, &blocked_row).is_err());
        assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
        assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), row);
    }
    let mut stale = row.clone();
    stale.version = 99;
    assert!(crate::build_record::compose(&mut sessions, &stale).is_err());
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), row);
    let receipt = crate::build_record::compose(&mut sessions, &row).unwrap();
    assert_eq!(receipt.revision, 1);
    let committed = sessions.store.load_job(&row.id).unwrap().unwrap();
    let (_, state) = decode(&committed).unwrap();
    assert_eq!(state.receipts.len(), 2);
    assert_eq!(state.expected_revision, 1);
    assert!(state.child_id.is_none());
    let (_, applied) = sessions.load("session").unwrap();
    assert_eq!(applied.history().project().clips.len(), 2);
    assert_eq!(
        applied.history().project().clips[0].id,
        input.plan.script.speech_ids[0]
    );
    assert_eq!(
        crate::build_record::compose(&mut sessions, &committed).unwrap(),
        receipt
    );
    assert_eq!(
        sessions.store.load_job(&row.id).unwrap().unwrap(),
        committed
    );
    drop(sessions);
    let mut sessions = Sessions::open(&path).unwrap();
    let recovered = sessions.store.load_job(&row.id).unwrap().unwrap();
    assert_eq!(decode(&recovered).unwrap().1.phase, JobPhase::Interrupted);
    assert_eq!(recovered.input, row.input);
    assert_eq!(
        crate::build_record::compose(&mut sessions, &recovered).unwrap(),
        receipt
    );
    let (stored, mut candidate) = sessions.load("session").unwrap();
    candidate
        .execute(EditRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: 1,
            action: EditAction::Undo {
                expected_operation_id: input.step_ids[1].clone(),
            },
        })
        .unwrap();
    sessions
        .store
        .commit(
            Some(stored.version),
            &StoredSession {
                id: stored.id,
                revision: 2,
                version: stored.version + 1,
                snapshot: candidate.snapshot().unwrap().into(),
            },
        )
        .unwrap();
    assert_eq!(
        crate::build_record::compose(&mut sessions, &recovered).unwrap(),
        receipt
    );
    assert_eq!(
        sessions.load("session").unwrap().1.history().project(),
        &original
    );
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
