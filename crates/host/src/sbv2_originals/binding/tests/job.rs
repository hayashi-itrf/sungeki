use super::startup::request;
use crate::{
    sbv2_job,
    sessions::Sessions,
    speech_dispatch::Reservation,
    speech_journal::{Input, create_input, decode_input},
    speech_journal_tests::{id, seed},
    speech_state::Execution,
};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_project::{JobPhase, Voice};
use sungeki_protocol::{EditAction, EditRequest, SessionRequest};
use sungeki_speech::sbv2::Sbv2JobInput;

pub(super) fn jobs(
    sessions: &mut Sessions,
    request: &crate::sbv2_startup::Request,
) -> (String, String) {
    let (sid, base) = seed(sessions);
    let mut project = base.project().clone();
    project.characters[0].voice = Voice::Sbv2 {
        settings: request.settings.clone(),
        identity: request.identity.clone(),
    };
    sessions
        .request(SessionRequest::Edit {
            session_id: sid.clone(),
            edit: EditRequest {
                operation_id: id(),
                expected_revision: 1,
                action: EditAction::Apply {
                    commands: vec![sungeki_editor::Command::ReplaceProject {
                        project: project.clone(),
                        checkpoint_id: None,
                    }],
                },
            },
        })
        .unwrap();
    let config = std::fs::read_to_string(&request.binding.originals.config).unwrap();
    let input = Input::Sbv2(
        Sbv2JobInput::new(
            2,
            &project,
            base.clip_id(),
            &request.binding.endpoint,
            &request.identity,
            &request.catalog,
            &config,
        )
        .unwrap(),
    );
    let first = create_input(sessions, id(), sid.clone(), &input).unwrap();
    let second = create_input(sessions, id(), sid, &input).unwrap();
    (first.id, second.id)
}
fn state(shared: &Arc<Mutex<Sessions>>, id: &str) -> crate::speech_state::State {
    decode_input(&shared.lock().unwrap().store.load_job(id).unwrap().unwrap())
        .unwrap()
        .1
}
#[tokio::test]
async fn running_engine_holds_resources_and_stop_persists_finished() {
    let (fixture, request, _) = request("ready");
    let mut sessions = fixture.open();
    let (first, second) = jobs(&mut sessions, &request);
    let shared = Arc::new(Mutex::new(sessions));
    let managed = sbv2_job::start(
        &shared,
        &first,
        request.binding,
        true,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap();
    assert_eq!(state(&shared, &first).execution, Execution::PossiblyRunning);
    assert!(Reservation::acquire(&mut shared.lock().unwrap(), &second).is_err());
    assert_eq!(managed.input.source_revision(), 2);
    managed.finish(&shared).await.unwrap();
    let record = state(&shared, &first);
    assert_eq!(record.execution, Execution::Finished);
    assert_eq!(record.phase, JobPhase::Running); // No WAV verification/application yet.
    assert!(Reservation::acquire(&mut shared.lock().unwrap(), &second).is_ok());
}
#[tokio::test]
async fn startup_failure_and_cancel_release_only_after_recording_settlement() {
    for cancel in [false, true] {
        let (fixture, request, _) = request("invalid");
        let mut sessions = fixture.open();
        let (first, second) = jobs(&mut sessions, &request);
        let shared = Arc::new(Mutex::new(sessions));
        assert!(
            sbv2_job::start(
                &shared,
                &first,
                request.binding,
                true,
                Arc::new(AtomicBool::new(cancel)),
                Duration::from_secs(3)
            )
            .await
            .is_err()
        );
        let record = state(&shared, &first);
        assert_eq!(record.execution, Execution::Finished);
        assert_eq!(
            record.phase,
            if cancel {
                JobPhase::Cancelled
            } else {
                JobPhase::Failed
            }
        );
        assert!(Reservation::acquire(&mut shared.lock().unwrap(), &second).is_ok());
    }
}
#[tokio::test]
async fn dropping_managed_engine_preserves_quarantine_across_restart() {
    let (fixture, request, _) = request("ready");
    let mut sessions = fixture.open();
    let (first, second) = jobs(&mut sessions, &request);
    let shared = Arc::new(Mutex::new(sessions));
    let managed = sbv2_job::start(
        &shared,
        &first,
        request.binding,
        true,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap();
    drop(managed);
    assert_eq!(state(&shared, &first).execution, Execution::PossiblyRunning);
    assert!(Reservation::acquire(&mut shared.lock().unwrap(), &second).is_err());
    drop(shared);
    let mut restored = fixture.open();
    let record = decode_input(&restored.store.load_job(&first).unwrap().unwrap())
        .unwrap()
        .1;
    assert_eq!(record.execution, Execution::PossiblyRunning);
    assert_eq!(record.phase, JobPhase::Interrupted);
    assert!(Reservation::acquire(&mut restored, &second).is_err());
}

#[tokio::test]
async fn durable_cancel_during_startup_wins_without_wake_flag() {
    let (fixture, request, pid) = request("delayed");
    let mut sessions = fixture.open();
    let (first, second) = jobs(&mut sessions, &request);
    let shared = Arc::new(Mutex::new(sessions));
    let cancelling = shared.clone();
    let id = first.clone();
    let task = tokio::spawn(async move {
        for _ in 0..100 {
            if pid.exists() {
                let mut sessions = cancelling.lock().unwrap();
                let stored = sessions.store.load_job(&id).unwrap().unwrap();
                let mut state = decode_input(&stored).unwrap().1;
                state.phase = JobPhase::Cancelling;
                crate::speech_journal::save(&mut sessions, &stored, &state).unwrap();
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("child did not start");
    });
    assert!(
        sbv2_job::start(
            &shared,
            &first,
            request.binding,
            true,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(3)
        )
        .await
        .is_err()
    );
    task.await.unwrap();
    assert_eq!(state(&shared, &first).phase, JobPhase::Cancelled);
    assert_eq!(state(&shared, &first).execution, Execution::Finished);
    assert!(Reservation::acquire(&mut shared.lock().unwrap(), &second).is_ok());
}
