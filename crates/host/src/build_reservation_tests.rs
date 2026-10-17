use super::*;
use crate::build_record::{Reservation, request_cancel, reserve};
use sungeki_editor::{Command, EditAction, EditRequest, EditSession};
use sungeki_project::Project;

pub(super) fn setup(ready: bool) -> (std::path::PathBuf, Sessions, StoredJob) {
    let root = std::env::temp_dir().join(format!("vvm-reserve-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let mut recipe = input().recipe;
    let image = root.join("source.png");
    std::fs::write(
        &image,
        include_bytes!("../../../tests/fixtures/build-source.png"),
    )
    .unwrap();
    let probe = sungeki_assets::probe_image(&image, Default::default()).unwrap();
    recipe.assets[0].locator = sungeki_project::AssetLocator::Absolute {
        path: image.to_string_lossy().into(),
    };
    recipe.assets[0].content_hash = Some(probe.sha256.clone());
    recipe.assets[0].probe = sungeki_project::AssetProbe::Image(probe);
    let input = Input::new(recipe, &root.join("制作検証.vvm.json")).unwrap();
    let candidate =
        EditSession::new(Project::empty(input.plan.project_id.clone(), "新規".into())).unwrap();
    sessions
        .store
        .commit(
            None,
            &StoredSession {
                id: "session".into(),
                revision: 0,
                version: 0,
                snapshot: candidate.snapshot().unwrap().into(),
            },
        )
        .unwrap();
    let mut row = job(&input, JobPhase::Queued);
    if ready {
        let mut prepared = state(&input, JobPhase::Running);
        prepared.receipts.truncate(1);
        prepared.expected_revision = 0;
        prepared.child_id = None;
        row.state = serde_json::to_vec(&prepared).unwrap();
    }
    sessions.store.create_job(&row).unwrap();
    (root, sessions, row)
}

#[test]
fn reserve_once_cancel_retains_child_and_restart_does_not_relaunch() {
    let (root, mut sessions, row) = setup(false);
    let before = sessions.store.load("session").unwrap().unwrap();
    let Reservation::New(reserved) = reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    let (input, state) = decode(&reserved).unwrap();
    assert_eq!(state.phase, JobPhase::Running);
    assert_eq!(state.child_id, Some(input.step_ids[0].clone()));
    assert_eq!(reserved.version, 1);
    assert_eq!(
        reserve(&mut sessions, &reserved).unwrap(),
        Reservation::Existing(reserved.clone())
    );
    assert!(reserve(&mut sessions, &row).is_err());
    assert!(request_cancel(&mut sessions, &row).is_err());
    let cancelled = request_cancel(&mut sessions, &reserved).unwrap();
    let (_, state) = decode(&cancelled).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelling);
    assert_eq!(state.child_id, Some(input.step_ids[0].clone()));
    assert_eq!(
        request_cancel(&mut sessions, &cancelled).unwrap(),
        cancelled
    );
    assert!(reserve(&mut sessions, &cancelled).is_err());
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    drop(sessions);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let restored = sessions.store.load_job(&row.id).unwrap().unwrap();
    let (_, state) = decode(&restored).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.child_id, Some(input.step_ids[0].clone()));
    assert!(reserve(&mut sessions, &restored).is_err());
    assert_eq!(request_cancel(&mut sessions, &restored).unwrap(), restored);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reserved_compose_advances_only_once_and_next_reservation_is_first_speech() {
    let (root, mut sessions, row) = setup(true);
    let Reservation::New(reserved) = reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    let (input, _) = decode(&reserved).unwrap();
    let receipt = crate::build_record::compose(&mut sessions, &reserved).unwrap();
    let composed = sessions.store.load_job(&row.id).unwrap().unwrap();
    assert_eq!(
        crate::build_record::compose(&mut sessions, &composed).unwrap(),
        receipt
    );
    assert!(reserve(&mut sessions, &reserved).is_err());
    let Reservation::New(speech) = reserve(&mut sessions, &composed).unwrap() else {
        panic!()
    };
    assert_eq!(
        decode(&speech).unwrap().1.child_id,
        Some(input.step_ids[2].clone())
    );
    assert_eq!(
        sessions
            .load("session")
            .unwrap()
            .1
            .history()
            .project()
            .clips
            .len(),
        2
    );
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn intervening_user_edit_prevents_reservation_without_rewriting_either_record() {
    let (root, mut sessions, row) = setup(true);
    let (stored, mut candidate) = sessions.load("session").unwrap();
    candidate
        .execute(EditRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: 0,
            action: EditAction::Apply {
                commands: vec![Command::RenameProject {
                    title: "ユーザーの編集".into(),
                }],
            },
        })
        .unwrap();
    let edited = StoredSession {
        id: stored.id,
        revision: 1,
        version: stored.version + 1,
        snapshot: candidate.snapshot().unwrap().into(),
    };
    sessions
        .store
        .commit(Some(stored.version), &edited)
        .unwrap();
    assert!(reserve(&mut sessions, &row).is_err());
    assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), row);
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), edited);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancellation_wins_against_late_compose_and_rolls_back_candidate_edit() {
    let (root, mut sessions, row) = setup(true);
    let before = sessions.store.load("session").unwrap().unwrap();
    let Reservation::New(reserved) = reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    let cancelling = request_cancel(&mut sessions, &reserved).unwrap();
    assert!(crate::build_record::compose(&mut sessions, &reserved).is_err());
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    assert_eq!(
        sessions.store.load_job(&row.id).unwrap().unwrap(),
        cancelling
    );
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
