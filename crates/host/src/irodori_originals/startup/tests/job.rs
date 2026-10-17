use super::{Request, fixture as request, gone as absent};
use crate::{
    sessions::Sessions,
    speech_journal::{Input, decode_input},
    speech_journal_tests::{Fixture, id, seed},
    speech_state::Execution,
};
use serde_json::json;
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_project::{ClipContent, JobPhase, Voice};
use sungeki_protocol::{EditAction, EditRequest, SessionRequest};
use sungeki_speech::irodori::{Catalog, IrodoriJobInput};

pub(super) fn seed_irodori(
    sessions: &mut Sessions,
    request: &Request,
) -> (String, IrodoriJobInput) {
    let (sid, original) = seed(sessions);
    let mut project = original.project().clone();
    project.characters[0].voice = Voice::Irodori {
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
    let catalog = Catalog::new(
        serde_json::from_value(json!({"checkpoint":"model","speaker_conditioning":true,"caption_conditioning":false,"sample_rate":48000,"channels":1})).unwrap(),
        serde_json::from_value(json!([{"id":"none","name":"参照なし","reference":"none","caption":null}])).unwrap(),
    ).unwrap();
    let input =
        IrodoriJobInput::new(2, &project, original.clip_id(), &request.identity, &catalog).unwrap();
    (sid, input)
}

#[tokio::test]
async fn managed_job_stops_applies_and_replays_without_relaunch() {
    let (_engine, request, pid) = request("speech");
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_irodori(&mut sessions, &request);
    let runtime = sessions.speech_runtime.clone();
    let shared = Arc::new(Mutex::new(sessions));
    let job = crate::speech_runtime::start_irodori(
        &shared,
        sid.clone(),
        id(),
        &input,
        request.binding.clone(),
        request.port,
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    absent(&pid);
    let completed = crate::speech_runtime::start_irodori(
        &shared,
        sid.clone(),
        job.id.clone(),
        &input,
        request.binding,
        request.port,
    )
    .unwrap();
    let (_, state) = decode_input(&completed).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded, "{:?}", state.error);
    assert_eq!(state.execution, Execution::Finished);
    assert!(state.artifact.is_some());
    assert_eq!(state.progress.as_ref().unwrap().stage, "speech.save_audio");
    assert_eq!(state.progress.as_ref().unwrap().fraction, None);
    assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
    drop(shared);
    let sessions = fixture.open();
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    let (_, session) = sessions.load(&sid).unwrap();
    let project = session.history().project();
    let ClipContent::Speech {
        audio: Some(audio), ..
    } = &project.clips[0].content
    else {
        panic!()
    };
    assert!(audio.irodori_source.is_some());
    assert_eq!(project.clips[0].duration_us.0, 10000);
    assert!(sungeki_speech::audio_reference_matches(project, input.clip_id()).unwrap());
}

#[tokio::test]
async fn durable_cancellation_stops_child_before_releasing_resources() {
    let (_engine, request, pid) = request("speech_hang");
    let marker = request.binding.work_root.join("speech.started");
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_irodori(&mut sessions, &request);
    let job = crate::speech_journal::create_input(
        &mut sessions,
        id(),
        sid.clone(),
        &Input::Irodori(input.clone()),
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let worker_shared = shared.clone();
    let job_id = job.id.clone();
    // No runtime control flag: the worker must observe the durable cancellation.
    let task = tokio::spawn(async move {
        crate::irodori_originals::job::run(
            &worker_shared,
            &job_id,
            request.binding,
            request.port,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            Duration::from_secs(10),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    {
        let mut sessions = shared.lock().unwrap();
        let record = sessions.store.load_job(&job.id).unwrap().unwrap();
        let (_, mut state) = decode_input(&record).unwrap();
        assert_eq!(state.execution, Execution::PossiblyRunning);
        state.phase = JobPhase::Cancelling;
        crate::speech_journal::save(&mut sessions, &record, &state).unwrap();
    }
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err()
            .code,
        sungeki_protocol::ErrorCode::Cancelled
    );
    absent(&pid);
    let sessions = shared.lock().unwrap();
    let record = sessions.store.load_job(&job.id).unwrap().unwrap();
    let (_, state) = decode_input(&record).unwrap();
    assert_eq!(state.phase, JobPhase::Cancelled);
    assert_eq!(state.execution, Execution::Finished);
    assert!(state.artifact.is_none());
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    assert_eq!(
        sessions.load(&sid).unwrap().1.history().project(),
        input.project()
    );
}

#[tokio::test]
async fn invalid_audio_fails_without_publishing_and_releases_settled_engine() {
    let (_engine, request, pid) = request("speech_bad");
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed_irodori(&mut sessions, &request);
    let job = crate::speech_journal::create_input(
        &mut sessions,
        id(),
        sid.clone(),
        &Input::Irodori(input.clone()),
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let error = crate::irodori_originals::job::run(
        &shared,
        &job.id,
        request.binding,
        request.port,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, sungeki_protocol::ErrorCode::ValidationFailed);
    assert_eq!(
        error.details.get("cleanup_confirmed").map(String::as_str),
        Some("true")
    );
    absent(&pid);
    let mut sessions = shared.lock().unwrap();
    let record = sessions.store.load_job(&job.id).unwrap().unwrap();
    let (_, state) = decode_input(&record).unwrap();
    assert_eq!(state.phase, JobPhase::Failed);
    assert_eq!(state.execution, Execution::Finished);
    assert!(state.artifact.is_none());
    assert_eq!(
        sessions.load(&sid).unwrap().1.history().project(),
        input.project()
    );
    let next =
        crate::speech_journal::create_input(&mut sessions, id(), sid, &Input::Irodori(input))
            .unwrap();
    drop(crate::speech_dispatch::Reservation::acquire(&mut sessions, &next.id).unwrap());
}
