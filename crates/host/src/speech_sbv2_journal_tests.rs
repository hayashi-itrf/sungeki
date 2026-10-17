use super::{Input, create_input, decode_input};
use crate::{
    sessions::Sessions,
    speech_dispatch::Reservation,
    speech_journal_tests::{Fixture, id, seed},
    speech_state::Execution,
};
use serde_json::json;
use sungeki_project::{JobPhase, Sbv2Identity, Voice};
use sungeki_protocol::{EditAction, EditRequest, SessionRequest};
use sungeki_speech::sbv2::{Catalog, ModelConfig, Sbv2JobInput};

pub(super) fn sbv2(sessions: &mut Sessions) -> (String, Input) {
    let (sid, original) = seed(sessions);
    let mut project = original.project().clone();
    let config = serde_json::to_string(&json!({"version":"2.7.0-JP-Extra",
        "data":{"sampling_rate":44100,"spk2id":{"声":0},"num_styles":1,
        "style2id":{"Neutral":0}}}))
    .unwrap();
    let identity = Sbv2Identity {
        engine_revision: "fixture".into(),
        model_sha256: "a".repeat(64),
        config_sha256: ModelConfig::parse(config.as_bytes())
            .unwrap()
            .sha256()
            .into(),
        style_vectors_sha256: "b".repeat(64),
        text_encoder_fingerprint: "c".repeat(64),
    };
    project.characters[0].voice = Voice::Sbv2 {
        identity: identity.clone(),
        settings: serde_json::from_value(json!({"model_id":0,"speaker_id":0,"language":"JP",
        "style":"Neutral","style_weight":1.,"length":1.,"sdp_ratio":0.2,
        "noise":0.6,"noisew":0.8,"auto_split":true,"split_interval":0.5}))
        .unwrap(),
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
    let catalog = Catalog::parse(json!({"0":{"model_path":"/model","config_path":"/config",
        "device":"cpu","spk2id":{"声":0},"id2spk":{"0":"声"},"style2id":{"Neutral":0}}}))
    .unwrap();
    let input = Sbv2JobInput::new(
        2,
        &project,
        original.clip_id(),
        "http://127.0.0.1:5050",
        &identity,
        &catalog,
        &config,
    )
    .unwrap();
    (sid, Input::Sbv2(input))
}
#[test]
fn sbv2_dispatch_quarantine_survives_restart_without_changing_project() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = sbv2(&mut sessions);
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
fn sbv2_unsent_drop_and_confirmed_completion_release_ownership() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = sbv2(&mut sessions);
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
fn sbv2_cancellation_preserves_unknown_remote_execution() {
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    use sungeki_protocol::{ApiError, ErrorCode};
    for dispatched in [false, true] {
        let fixture = Fixture::new();
        let mut sessions = fixture.open();
        let (sid, input) = sbv2(&mut sessions);
        let record = create_input(&mut sessions, id(), sid, &input).unwrap();
        if dispatched {
            drop(
                Reservation::acquire(&mut sessions, &record.id)
                    .unwrap()
                    .begin(&mut sessions)
                    .unwrap(),
            );
        }
        let shared = Arc::new(Mutex::new(sessions));
        crate::speech_api::execute(
            &shared,
            sungeki_protocol::SpeechJobRequest::Cancel {
                job_id: record.id.clone(),
            },
        )
        .unwrap();
        crate::speech_failure::record(
            &shared,
            &record.id,
            ApiError::new(ErrorCode::Cancelled, "検証"),
            &AtomicBool::new(true),
        )
        .unwrap();
        let sessions = shared.lock().unwrap();
        let record = sessions.store.load_job(&record.id).unwrap().unwrap();
        let (_, state) = decode_input(&record).unwrap();
        assert_eq!(
            state.phase,
            if dispatched {
                JobPhase::Interrupted
            } else {
                JobPhase::Cancelled
            }
        );
        assert_eq!(
            state.execution,
            if dispatched {
                Execution::PossiblyRunning
            } else {
                Execution::NotDispatched
            }
        );
        assert_eq!(
            sessions.speech_resources.quarantined().unwrap().len(),
            usize::from(dispatched)
        );
    }
}

#[test]
fn sbv2_audio_uses_common_verified_original_and_lipsync_storage() {
    use std::sync::atomic::AtomicBool;
    use sungeki_speech::SpeechResponse;
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (_, Input::Sbv2(input)) = sbv2(&mut sessions) else {
        panic!()
    };
    let prepared = input.prepared().unwrap();
    let root = sessions.speech_directory.as_ref().unwrap();
    let mut bytes = crate::speech_output_tests::response().unverified_wav;
    // Same 24 PCM samples, in the rate declared by the SBV2 model config.
    bytes[24..28].copy_from_slice(&44100u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&88200u32.to_le_bytes());
    let job = id();
    let artifact = crate::speech_artifacts::store(
        root,
        &job,
        SpeechResponse::sbv2(prepared.audio(), bytes.clone()).unwrap(),
        input.lip_sync_settings(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(std::fs::read(&artifact.path).unwrap(), bytes);
    assert_eq!(
        (
            artifact.sample_rate,
            artifact.channels,
            artifact.frames,
            artifact.duration_us
        ),
        (44100, 1, 24, 545)
    );
    let timing = artifact.lip_sync.as_ref().unwrap();
    assert_eq!(timing.source_sha256, artifact.sha256);
    crate::speech_original::load(&artifact).unwrap();
    // Exclusive publication must not overwrite an existing original.
    assert!(
        crate::speech_artifacts::store(
            root,
            &job,
            SpeechResponse::sbv2(prepared.audio(), bytes.clone()).unwrap(),
            input.lip_sync_settings(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&artifact.path).unwrap(), bytes);
    for (bad, cancel) in [
        (crate::speech_output_tests::response().unverified_wav, false),
        (b"not-wav".to_vec(), false),
        (bytes, true),
    ] {
        let job = id();
        assert!(
            crate::speech_artifacts::store(
                root,
                &job,
                SpeechResponse::sbv2(prepared.audio(), bad).unwrap(),
                input.lip_sync_settings(),
                &AtomicBool::new(cancel)
            )
            .is_err()
        );
        assert!(!root.join(job).exists());
    }
}
