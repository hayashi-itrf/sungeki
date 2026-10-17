//! End-to-end host worker with a fake HTTP engine and actual WAV validation/DB.
use super::{job::jobs, startup::request};
use crate::{
    sbv2_exchange,
    sessions::Sessions,
    speech_dispatch::Reservation,
    speech_journal::decode_input,
    speech_state::{Application, Execution},
};
use std::{
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::Duration,
};
use sungeki_project::{ClipContent, JobPhase};
use sungeki_protocol::{EditAction, EditRequest, ErrorCode, SessionRequest};

fn stopped(path: &std::path::Path) {
    let pid: i32 = std::fs::read_to_string(path).unwrap().parse().unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
#[tokio::test]
async fn synthesis_stops_engine_before_common_storage_and_application() {
    let (fixture, request, pid) = request("speech");
    let mut sessions = fixture.open();
    let (first, second) = jobs(&mut sessions, &request);
    let shared = Arc::new(Mutex::new(sessions));
    sbv2_exchange::run(
        &shared,
        &first,
        request.binding,
        true,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap();
    stopped(&pid);
    let sent = std::fs::read_to_string(fixture.0.join("voice.request")).unwrap();
    assert!(sent.starts_with("POST /voice?text="));
    let mut sessions = shared.lock().unwrap();
    let stored = sessions.store.load_job(&first).unwrap().unwrap();
    let (input, state) = decode_input(&stored).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert_eq!(state.progress.as_ref().unwrap().stage, "speech.save_audio");
    assert_eq!(state.progress.as_ref().unwrap().fraction, None);
    assert_eq!(state.execution, Execution::Finished);
    assert!(matches!(state.application, Application::Applied { .. }));
    let artifact = state.artifact.unwrap();
    assert_eq!(
        std::fs::read(artifact.path).unwrap(),
        std::fs::read(fixture.0.join("response.wav")).unwrap()
    );
    let (_, session) = sessions.load(&stored.session_id).unwrap();
    let project = session.history().project();
    assert!(sungeki_speech::audio_reference_matches(project, input.clip_id()).unwrap());
    assert!(Reservation::acquire(&mut sessions, &second).is_ok());
    drop(sessions);
    let tracker = crate::speech_state::progress::Tracker::default();
    tracker.set(crate::speech_state::progress::Stage::Start);
    tracker.persist(&shared, &first).unwrap();
    let retained = shared
        .lock()
        .unwrap()
        .store
        .load_job(&first)
        .unwrap()
        .unwrap();
    assert_eq!(stored.version, retained.version);
    assert_eq!(stored.state, retained.state);
}
#[tokio::test]
async fn invalid_audio_changed_original_and_timeout_do_not_apply() {
    for mode in [
        "speech_bad_wav",
        "speech_wrong_rate",
        "speech_changed",
        "speech_hang",
    ] {
        let (fixture, request, pid) = request(mode);
        let mut sessions = fixture.open();
        let (first, second) = jobs(&mut sessions, &request);
        let shared = Arc::new(Mutex::new(sessions));
        let error = sbv2_exchange::run(
            &shared,
            &first,
            request.binding,
            true,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
        stopped(&pid);
        assert!(
            fixture.0.join("voice.request").exists(),
            "must reach synthesis: {mode}"
        );
        if mode == "speech_changed" {
            assert_eq!(error.code, ErrorCode::ExternalModification);
        }
        let mut sessions = shared.lock().unwrap();
        let stored = sessions.store.load_job(&first).unwrap().unwrap();
        let (input, state) = decode_input(&stored).unwrap();
        assert_eq!(state.phase, JobPhase::Failed);
        assert_eq!(state.execution, Execution::Finished);
        assert!(state.artifact.is_none());
        assert_eq!(
            sessions
                .load(&stored.session_id)
                .unwrap()
                .1
                .history()
                .project(),
            input.project()
        );
        assert!(Reservation::acquire(&mut sessions, &second).is_ok());
    }
}
async fn after_voice(path: &std::path::Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("voice request did not arrive");
}
#[tokio::test]
async fn durable_cancel_stops_inflight_synthesis_without_waiting_for_response() {
    let (fixture, request, pid) = request("speech_hang");
    let mut sessions = fixture.open();
    let (first, second) = jobs(&mut sessions, &request);
    let shared = Arc::new(Mutex::new(sessions));
    let cancelling = shared.clone();
    let id = first.clone();
    let path = fixture.0.join("voice.request");
    let task = tokio::spawn(async move {
        after_voice(&path).await;
        crate::speech_runtime::cancel(&cancelling, &id).unwrap();
    });
    let started = std::time::Instant::now();
    let error = sbv2_exchange::run(
        &shared,
        &first,
        request.binding,
        true,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(30),
    )
    .await
    .unwrap_err();
    task.await.unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    stopped(&pid);
    assert_eq!(error.code, ErrorCode::Cancelled);
    let mut sessions = shared.lock().unwrap();
    let (_, state) = decode_input(&sessions.store.load_job(&first).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelled);
    assert_eq!(state.execution, Execution::Finished);
    assert!(state.artifact.is_none());
    assert!(Reservation::acquire(&mut sessions, &second).is_ok());
}
#[tokio::test]
async fn concurrent_dialogue_edit_preserves_pending_verified_audio() {
    let (fixture, request, pid) = request("speech_delayed");
    let mut sessions = fixture.open();
    let (first, _) = jobs(&mut sessions, &request);
    let shared = Arc::new(Mutex::new(sessions));
    let editing = shared.clone();
    let id = first.clone();
    let path = fixture.0.join("voice.request");
    let task = tokio::spawn(async move {
        after_voice(&path).await;
        let mut sessions = editing.lock().unwrap();
        let stored = sessions.store.load_job(&id).unwrap().unwrap();
        let (input, _) = decode_input(&stored).unwrap();
        let mut project = input.project().clone();
        let ClipContent::Speech { subtitle_text, .. } = &mut project.clips[0].content else {
            panic!()
        };
        *subtitle_text = "合成中に編集".into();
        sessions
            .request(SessionRequest::Edit {
                session_id: stored.session_id,
                edit: EditRequest {
                    operation_id: crate::speech_journal_tests::id(),
                    expected_revision: 2,
                    action: EditAction::Apply {
                        commands: vec![sungeki_editor::Command::ReplaceProject {
                            project,
                            checkpoint_id: None,
                        }],
                    },
                },
            })
            .unwrap();
    });
    sbv2_exchange::run(
        &shared,
        &first,
        request.binding,
        true,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(3),
    )
    .await
    .unwrap();
    task.await.unwrap();
    stopped(&pid);
    let sessions = shared.lock().unwrap();
    let stored = sessions.store.load_job(&first).unwrap().unwrap();
    let (_, state) = decode_input(&stored).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert!(matches!(state.application, Application::PendingConflict));
    assert!(state.artifact.is_some());
    let (_, session) = sessions.load(&stored.session_id).unwrap();
    let ClipContent::Speech {
        subtitle_text,
        audio,
        ..
    } = &session.history().project().clips[0].content
    else {
        panic!()
    };
    assert_eq!(subtitle_text, "合成中に編集");
    assert!(audio.is_none());
}
