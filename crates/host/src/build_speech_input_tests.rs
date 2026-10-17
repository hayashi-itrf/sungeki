use super::*;
use crate::build_record::{Reservation, request_cancel, reserve};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_project::Voice;
use sungeki_speech::{SpeechJobInput, VoicevoxIdentity};

pub(super) fn setup() -> (
    std::path::PathBuf,
    Sessions,
    StoredJob,
    SpeechJobInput,
    String,
) {
    let (root, mut sessions, row) = super::reservation_tests::setup(true);
    let Reservation::New(compose) = reserve(&mut sessions, &row).unwrap() else {
        panic!()
    };
    crate::build_record::compose(&mut sessions, &compose).unwrap();
    let composed = sessions.store.load_job(&row.id).unwrap().unwrap();
    let Reservation::New(parent) = reserve(&mut sessions, &composed).unwrap() else {
        panic!()
    };
    let (plan, state) = decode(&parent).unwrap();
    let (_, current) = sessions.load(&parent.session_id).unwrap();
    let project = current.history().project();
    let Voice::Voicevox {
        engine_id,
        core_version,
        ..
    } = &project.characters[0].voice
    else {
        panic!("VOICEVOX fixture required");
    };
    let input = SpeechJobInput::new(
        state.expected_revision,
        project,
        &plan.plan.script.speech_ids[0],
        "http://127.0.0.1:55021",
        &VoicevoxIdentity {
            engine_id: engine_id.clone(),
            core_version: core_version.clone(),
            engine_version: "1.0".into(),
        },
    )
    .unwrap()
    .for_parent(&parent.id)
    .unwrap();
    (root, sessions, parent, input, state.child_id.unwrap())
}

#[test]
fn parent_reservation_is_checked_before_creation_and_again_before_dispatch() {
    let (root, mut sessions, parent, input, child_id) = setup();
    assert!(
        crate::speech_journal::create(
            &mut sessions,
            uuid::Uuid::new_v4().to_string(),
            parent.session_id.clone(),
            &input
        )
        .is_err()
    );
    assert!(
        crate::speech_journal::create(
            &mut sessions,
            child_id.clone(),
            "other-session".into(),
            &input
        )
        .is_err()
    );
    let child = crate::speech_journal::create(
        &mut sessions,
        child_id.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    let reserved = crate::speech_dispatch::Reservation::acquire(&mut sessions, &child_id).unwrap();
    request_cancel(&mut sessions, &parent).unwrap();
    assert!(reserved.begin(&mut sessions).is_err());
    assert_eq!(sessions.store.load_job(&child_id).unwrap().unwrap(), child);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn verified_parent_audio_is_stored_without_independent_project_application() {
    let (root, mut sessions, parent, input, child_id) = setup();
    let before = sessions.store.load(&parent.session_id).unwrap().unwrap();
    crate::speech_journal::create(
        &mut sessions,
        child_id.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    crate::speech_dispatch::Reservation::acquire(&mut sessions, &child_id)
        .unwrap()
        .begin(&mut sessions)
        .unwrap()
        .confirm_finished(&mut sessions)
        .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    crate::speech_output::complete(
        &shared,
        &child_id,
        crate::speech_output_tests::response(),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();
    {
        let sessions = shared.lock().unwrap();
        let child = sessions.store.load_job(&child_id).unwrap().unwrap();
        let (bound, state) = crate::speech_journal::decode(&child).unwrap();
        assert_eq!(bound.parent_job_id(), Some(parent.id.as_str()));
        assert_eq!(state.phase, JobPhase::Succeeded);
        assert!(matches!(
            state.application,
            crate::speech_state::Application::PendingConflict
        ));
        let artifact = state.artifact.unwrap();
        assert!(std::path::Path::new(&artifact.path).is_file());
        assert_eq!(
            sessions.store.load(&parent.session_id).unwrap().unwrap(),
            before
        );
        assert_eq!(
            sessions.store.load_job(&parent.id).unwrap().unwrap(),
            parent
        );
    }
    let replay =
        crate::build_record::start_speech(&shared, &parent, input.endpoint(), input.identity())
            .unwrap();
    let again =
        crate::build_record::start_speech(&shared, &parent, input.endpoint(), input.identity())
            .unwrap();
    assert_eq!(replay, again);
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .speech_runtime
            .active
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    drop(shared);
    let sessions = Sessions::open(&root.join("db")).unwrap();
    let (_, state) =
        crate::speech_journal::decode(&sessions.store.load_job(&child_id).unwrap().unwrap())
            .unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        before
    );
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "build_managed_parent_tests.rs"]
mod managed_parent_tests;

#[path = "build_preparation_tests.rs"]
mod preparation_tests;
