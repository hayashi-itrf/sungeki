use super::*;
use crate::build_record::{Reservation, reserve};
use std::sync::{Arc, Mutex};

#[test]
fn create_prepare_compose_runs_with_real_image_and_fixed_receipts() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let mut recipe = decode(&fixture).unwrap().0.recipe;
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    let parent = crate::build_record::create(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("新規準備.vvm.json"),
    )
    .unwrap();
    let Reservation::New(reserved) = reserve(&mut sessions, &parent).unwrap() else {
        panic!()
    };
    let before = sessions.store.load(&parent.session_id).unwrap().unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    crate::build_record::prepare(&shared, &reserved).unwrap();
    let prepared = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    let (_, state) = decode(&prepared).unwrap();
    assert_eq!(state.receipts.len(), 1);
    assert_eq!(state.expected_revision, 0);
    assert!(state.child_id.is_none());
    crate::build_record::prepare(&shared, &prepared).unwrap();
    let mut sessions = shared.lock().unwrap();
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        before
    );
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        prepared
    );
    let Reservation::New(compose) = reserve(&mut sessions, &prepared).unwrap() else {
        panic!()
    };
    crate::build_record::compose(&mut sessions, &compose).unwrap();
    assert_eq!(
        sessions
            .load(&parent.session_id)
            .unwrap()
            .1
            .history()
            .project()
            .clips
            .len(),
        2
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn invalid_preset_or_cancel_cannot_finish_preparation() {
    let (root, mut sessions, parent) = super::reservation_tests::setup(false);
    let Reservation::New(reserved) = reserve(&mut sessions, &parent).unwrap() else {
        panic!()
    };
    let shared = Arc::new(Mutex::new(sessions));
    assert!(crate::build_record::prepare(&shared, &reserved).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        reserved
    );
    let cancelled =
        crate::build_record::request_cancel(&mut shared.lock().unwrap(), &reserved).unwrap();
    assert!(crate::build_record::prepare(&shared, &cancelled).is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        cancelled
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn oversized_diagram_motion_stops_before_speech_without_committing_staging() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let mut recipe = decode(&fixture).unwrap().0.recipe;
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    recipe.scenes[0].diagrams[0].motion = Some(sungeki_project::VisualMotion {
        end_x: 400.,
        end_y: 200.,
        end_scale: 100000.,
        easing: sungeki_project::MotionEasing::Linear,
    });
    let parent = crate::build_record::create(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("過大な説明図.vvm.json"),
    )
    .unwrap();
    let Reservation::New(reserved) = reserve(&mut sessions, &parent).unwrap() else {
        panic!()
    };
    let shared = Arc::new(Mutex::new(sessions));
    crate::build_record::prepare(&shared, &reserved).unwrap();
    let mut sessions = shared.lock().unwrap();
    let prepared = sessions.store.load_job(&parent.id).unwrap().unwrap();
    let Reservation::New(compose) = reserve(&mut sessions, &prepared).unwrap() else {
        panic!()
    };
    let before = sessions.store.load(&parent.session_id).unwrap().unwrap();
    assert!(crate::build_record::compose(&mut sessions, &compose).is_err());
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        before
    );
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        compose
    );
    assert_eq!(decode(&compose).unwrap().1.receipts.len(), 1);
    assert!(!root.join("過大な説明図.vvm.json").exists());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
