use super::{Input, create_input, decode_input};
use crate::{
    sessions::Sessions,
    speech_dispatch::Reservation,
    speech_journal_tests::{Fixture, id, seed},
    speech_state::Execution,
};
use serde_json::json;
use sungeki_project::{JobPhase, Voice};
use sungeki_protocol::{EditAction, EditRequest, SessionRequest};
use sungeki_speech::qwen::{Catalog, QwenJobInput};

pub(super) fn qwen(sessions: &mut Sessions) -> (String, Input) {
    let (sid, original) = seed(sessions);
    let mut project = original.project().clone();
    let identity = sungeki_project::QwenIdentity {
        backend: "official".into(),
        model_id: "/ssd/qwen".into(),
        model_manifest_sha256: "a".repeat(64),
        engine_manifest_sha256: "b".repeat(64),
        voice_manifest_sha256: "c".repeat(64),
        sample_rate: 24000,
        channels: 1,
    };
    project.characters[0].voice = Voice::Qwen {
        identity: identity.clone(),
        settings: sungeki_project::QwenSettings {
            model: "qwen3-tts".into(),
            voice: "Vivian".into(),
            language: "Japanese".into(),
            speed: 1.,
            instruct: None,
        },
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
    let catalog=Catalog::parse(json!({"status":"healthy","backend":{"name":"official","model_id":"/ssd/qwen","ready":true},"version":"test"}),json!({"data":[{"id":"qwen3-tts"}]}),json!({"voices":[{"id":"Vivian","name":"Vivian"}],"languages":["Japanese"]})).unwrap();
    let input = QwenJobInput::new(
        2,
        &project,
        original.clip_id(),
        "http://127.0.0.1:5050",
        &identity,
        &catalog,
    )
    .unwrap();
    (sid, Input::Qwen(input))
}
#[test]
fn qwen_dispatch_quarantine_survives_restart_without_changing_project() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = qwen(&mut sessions);
    let first = create_input(&mut sessions, id(), sid.clone(), &input).unwrap();
    assert_eq!(
        decode_input(&first).unwrap().0.input_hash(),
        input.input_hash()
    );
    // VOICEVOX-only consumers fail explicitly; they never reinterpret SBV2.
    assert!(super::decode(&first).is_err());
    let reserved = Reservation::acquire(&mut sessions, &first.id).unwrap();
    let second = create_input(&mut sessions, id(), sid.clone(), &input).unwrap();
    assert!(Reservation::acquire(&mut sessions, &second.id).is_err());
    drop(reserved.begin(&mut sessions).unwrap());
    drop(sessions);
    let mut sessions = fixture.open();
    let record = sessions.store.load_job(&first.id).unwrap().unwrap();
    let (_, state) = decode_input(&record).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.execution, Execution::PossiblyRunning);
    assert!(Reservation::acquire(&mut sessions, &second.id).is_err());
    assert_eq!(
        sessions.load(&sid).unwrap().1.history().project(),
        input.project()
    );
    assert_eq!(sessions.speech_resources.quarantined().unwrap().len(), 1);
}
#[test]
fn qwen_unsent_drop_and_confirmed_completion_release_ownership() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = qwen(&mut sessions);
    let first = create_input(&mut sessions, id(), sid.clone(), &input).unwrap();
    drop(Reservation::acquire(&mut sessions, &first.id).unwrap());
    let dispatch = Reservation::acquire(&mut sessions, &first.id)
        .unwrap()
        .begin(&mut sessions)
        .unwrap();
    dispatch.confirm_finished(&mut sessions).unwrap();
    let record = sessions.store.load_job(&first.id).unwrap().unwrap();
    assert_eq!(
        decode_input(&record).unwrap().1.execution,
        Execution::Finished
    );
    let second = create_input(&mut sessions, id(), sid, &input).unwrap();
    drop(Reservation::acquire(&mut sessions, &second.id).unwrap());
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
}
#[test]
fn qwen_response_is_not_accepted_as_voicevox_and_must_match_captured_input() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (_, original) = seed(&mut sessions);
    let (_, input) = qwen(&mut sessions);
    let Input::Qwen(qwen) = &input else { panic!() };
    let prepared = qwen.prepared().unwrap();
    let response = sungeki_speech::SpeechResponse::qwen(
        prepared.audio(),
        crate::speech_output_tests::response().unverified_wav,
    )
    .unwrap();
    assert!(input.accepts_response(&response).unwrap());
    assert!(
        !Input::Voicevox(original)
            .accepts_response(&response)
            .unwrap()
    );
}
