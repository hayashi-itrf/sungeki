use crate::{
    speech_dispatch::Reservation,
    speech_journal::{create, decode, save},
    speech_journal_tests::{Fixture, id, seed},
    speech_state::Execution,
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, ErrorCode};

#[test]
fn reservation_excludes_competitors_and_unsent_drop_releases_without_state_change() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let first = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    let second = create(&mut sessions, id(), sid, &input).unwrap();
    let reserved = Reservation::acquire(&mut sessions, &first.id).unwrap();
    assert_eq!(reserved.input().input_hash(), input.input_hash());
    assert!(Reservation::acquire(&mut sessions, &first.id).is_err());
    assert!(Reservation::acquire(&mut sessions, &second.id).is_err());
    assert_eq!(sessions.store.load_job(&first.id).unwrap().unwrap(), first);
    drop(reserved);
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    drop(Reservation::acquire(&mut sessions, &second.id).unwrap());
    assert_eq!(
        sessions.store.load_job(&second.id).unwrap().unwrap(),
        second
    );
}

#[test]
fn cancellation_during_preparation_prevents_dispatch_and_releases_reserved_slots() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let first = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    let reserved = Reservation::acquire(&mut sessions, &first.id).unwrap();
    let (_, mut state) = decode(&first).unwrap();
    state.phase = JobPhase::Cancelling;
    save(&mut sessions, &first, &state).unwrap();
    assert!(reserved.begin(&mut sessions).is_err());
    let record = sessions.store.load_job(&first.id).unwrap().unwrap();
    let (_, state) = decode(&record).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelling);
    assert_eq!(state.execution, Execution::NotDispatched);
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    let second = create(&mut sessions, id(), sid, &input).unwrap();
    drop(Reservation::acquire(&mut sessions, &second.id).unwrap());
}

#[test]
fn dropped_dispatch_is_durable_and_blocks_other_work_before_and_after_recovery() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let first = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    let dispatched = Reservation::acquire(&mut sessions, &first.id)
        .unwrap()
        .begin(&mut sessions)
        .unwrap();
    let (_, state) = decode(&sessions.store.load_job(&first.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Running);
    assert_eq!(state.execution, Execution::PossiblyRunning);
    drop(dispatched); // same path as a worker cancellation/panic after dispatch
    let second = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    assert!(Reservation::acquire(&mut sessions, &second.id).is_err());
    assert_eq!(sessions.speech_resources.quarantined().unwrap().len(), 1);
    drop(sessions);
    let mut sessions = fixture.open();
    let (_, state) = decode(&sessions.store.load_job(&first.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.execution, Execution::PossiblyRunning);
    let third = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    assert!(Reservation::acquire(&mut sessions, &third.id).is_err());
    assert_eq!(
        sessions.load(&sid).unwrap().1.history().project(),
        input.project()
    );
}

#[test]
fn completion_is_durable_before_release_and_preserves_cancellation_or_failure() {
    for phase in [JobPhase::Running, JobPhase::Cancelling, JobPhase::Failed] {
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = seed(&mut sessions);
        let first = create(&mut sessions, id(), sid.clone(), &input).unwrap();
        let dispatched = Reservation::acquire(&mut sessions, &first.id)
            .unwrap()
            .begin(&mut sessions)
            .unwrap();
        let record = sessions.store.load_job(&first.id).unwrap().unwrap();
        let (_, mut state) = decode(&record).unwrap();
        state.phase = phase;
        if phase == JobPhase::Failed {
            state.error = Some(ApiError::new(
                ErrorCode::EngineUnavailable,
                "応答の検証失敗",
            ));
        }
        save(&mut sessions, &record, &state).unwrap();
        dispatched.confirm_finished(&mut sessions).unwrap();
        let (_, state) = decode(&sessions.store.load_job(&first.id).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, phase);
        assert_eq!(state.execution, Execution::Finished);
        assert_eq!(state.error.is_some(), phase == JobPhase::Failed);
        let second = create(&mut sessions, id(), sid, &input).unwrap();
        drop(Reservation::acquire(&mut sessions, &second.id).unwrap());
        assert!(Reservation::acquire(&mut sessions, &first.id).is_err());
        drop(sessions);
        let sessions = fixture.open();
        assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
        let (_, state) = decode(&sessions.store.load_job(&first.id).unwrap().unwrap()).unwrap();
        assert_eq!(state.execution, Execution::Finished);
        assert!(state.phase.is_terminal());
    }
}

#[test]
fn persistence_failures_never_authorize_dispatch_or_release_running_work() {
    for fail_at_finish in [false, true] {
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = seed(&mut sessions);
        let first = create(&mut sessions, id(), sid.clone(), &input).unwrap();
        let reserved = Reservation::acquire(&mut sessions, &first.id).unwrap();
        let fault = rusqlite::Connection::open(fixture.0.join("recovery.sqlite")).unwrap();
        let install_fault = || {
            fault
                .execute_batch(
                    "CREATE TRIGGER fail_speech_update BEFORE UPDATE ON jobs
                BEGIN SELECT RAISE(ABORT, 'injected durable write failure'); END;",
                )
                .unwrap();
        };
        if fail_at_finish {
            let dispatched = reserved.begin(&mut sessions).unwrap();
            install_fault();
            assert_eq!(
                dispatched.confirm_finished(&mut sessions).unwrap_err().code,
                ErrorCode::IoError
            );
        } else {
            install_fault();
            assert!(reserved.begin(&mut sessions).is_err());
        }
        fault
            .execute_batch("DROP TRIGGER fail_speech_update;")
            .unwrap();
        assert_eq!(sessions.speech_resources.quarantined().unwrap().len(), 1);
        let (_, state) = decode(&sessions.store.load_job(&first.id).unwrap().unwrap()).unwrap();
        assert_eq!(
            state.execution,
            if fail_at_finish {
                Execution::PossiblyRunning
            } else {
                Execution::NotDispatched
            }
        );
        let second = create(&mut sessions, id(), sid, &input).unwrap();
        assert!(Reservation::acquire(&mut sessions, &second.id).is_err());
        drop(sessions);
        let sessions = fixture.open();
        // Failed begin never returned permission to send HTTP. Failed finish
        // retains the durable unknown-execution marker across a process restart.
        assert_eq!(
            sessions.speech_resources.quarantined().unwrap().len(),
            usize::from(fail_at_finish)
        );
    }
}
