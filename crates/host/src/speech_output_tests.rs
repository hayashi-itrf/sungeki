use crate::{
    sessions::Sessions,
    speech_dispatch::Reservation,
    speech_journal::{create, decode},
    speech_journal_tests::{Fixture, id, seed},
    speech_output::complete,
    speech_state::Execution,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};
use sungeki_project::JobPhase;
use sungeki_protocol::ErrorCode;
use sungeki_speech::voicevox::VoicevoxResponse;

pub(crate) fn response() -> VoicevoxResponse {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(84u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(24000u32.to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(48u32.to_le_bytes());
    bytes.extend([0; 48]);
    VoicevoxResponse {
        query: serde_json::from_value(serde_json::json!({
            "accent_phrases":[],"speedScale":1,"pitchScale":0,"intonationScale":1,"volumeScale":1,
            "prePhonemeLength":0.1,"postPhonemeLength":0.1,"outputSamplingRate":24000,"outputStereo":false
        })).unwrap(), unverified_wav: bytes,
    }
}
fn setup() -> (Fixture, Arc<Mutex<Sessions>>, String, String, PathBuf) {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let job = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    Reservation::acquire(&mut sessions, &job.id)
        .unwrap()
        .begin(&mut sessions)
        .unwrap()
        .confirm_finished(&mut sessions)
        .unwrap();
    let destination = sessions
        .speech_directory
        .as_ref()
        .unwrap()
        .join(&job.id)
        .join("audio.wav");
    (
        fixture,
        Arc::new(Mutex::new(sessions)),
        job.id,
        sid,
        destination,
    )
}
#[tokio::test]
async fn success_applies_verified_original_and_duration_atomically() {
    let (fixture, shared, job, sid, path) = setup();
    let original = shared
        .lock()
        .unwrap()
        .load(&sid)
        .unwrap()
        .1
        .history()
        .project()
        .clone();
    complete(&shared, &job, response(), &AtomicBool::new(false))
        .await
        .unwrap();
    let expected = response().verify().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), expected.audio.bytes());
    {
        let sessions = shared.lock().unwrap();
        let (_, state) = decode(&sessions.store.load_job(&job).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, JobPhase::Succeeded);
        let artifact = state.artifact.unwrap();
        assert_eq!(artifact.path, path.to_str().unwrap());
        assert_eq!(artifact.sha256, expected.audio.info().content_hash);
        let timing = artifact.lip_sync.as_ref().unwrap();
        assert_eq!(timing.source_sha256, artifact.sha256);
        assert!(timing.open.is_empty());
        assert_eq!(
            (artifact.duration_us, artifact.frames, artifact.byte_len),
            (1000, 24, 92)
        );
        let (_, applied) = sessions.load(&sid).unwrap();
        let project = applied.history().project();
        assert_eq!(project.clips[0].duration_us.0, 1000);
        assert_eq!(project.assets.len(), 1);
        let sungeki_project::ClipContent::Speech {
            audio: Some(audio), ..
        } = &project.clips[0].content
        else {
            panic!()
        };
        assert_eq!(audio.lip_sync, artifact.lip_sync);
        assert!(sungeki_speech::audio_reference_matches(project, &project.clips[0].id).unwrap());
        assert!(matches!(
            state.application,
            crate::speech_state::Application::Applied { revision: 2, .. }
        ));
        assert_eq!(project.characters, original.characters);
    }
    assert_eq!(
        complete(&shared, &job, response(), &AtomicBool::new(false))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    drop(shared);
    let sessions = fixture.open();
    assert_eq!(
        decode(&sessions.store.load_job(&job).unwrap().unwrap())
            .unwrap()
            .1
            .phase,
        JobPhase::Succeeded
    );
    assert_eq!(std::fs::read(path).unwrap(), expected.audio.bytes());
}
#[tokio::test]
async fn invalid_audio_and_preexisting_directory_never_become_success_or_overwrite() {
    for preexisting in [false, true] {
        let (_fixture, shared, job, _, path) = setup();
        let mut result = response();
        if preexisting {
            std::fs::create_dir(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"keep original").unwrap();
        } else {
            result.unverified_wav.truncate(50);
        }
        assert!(
            complete(&shared, &job, result, &AtomicBool::new(false))
                .await
                .is_err()
        );
        let sessions = shared.lock().unwrap();
        let (_, state) = decode(&sessions.store.load_job(&job).unwrap().unwrap()).unwrap();
        assert_eq!(state.phase, JobPhase::Failed);
        assert_eq!(state.execution, Execution::Finished);
        assert!(state.artifact.is_none());
        if preexisting {
            assert_eq!(std::fs::read(path).unwrap(), b"keep original");
        } else {
            assert!(!path.parent().unwrap().exists());
        }
    }
}
#[tokio::test]
async fn database_failure_preserves_file_and_recovers_without_false_success() {
    let (fixture, shared, job, _, path) = setup();
    let fault = rusqlite::Connection::open(fixture.0.join("recovery.sqlite")).unwrap();
    fault.execute_batch("CREATE TRIGGER fail_output BEFORE UPDATE ON jobs BEGIN SELECT RAISE(ABORT, 'fail result registration'); END;").unwrap();
    assert!(
        complete(&shared, &job, response(), &AtomicBool::new(false))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), response().unverified_wav);
    let (_, state) = decode(
        &shared
            .lock()
            .unwrap()
            .store
            .load_job(&job)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.phase, JobPhase::Running);
    assert!(state.artifact.is_none());
    fault.execute_batch("DROP TRIGGER fail_output;").unwrap();
    drop(shared);
    let sessions = fixture.open();
    let (_, state) = decode(&sessions.store.load_job(&job).unwrap().unwrap()).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.execution, Execution::Finished);
    assert!(state.artifact.is_none());
    assert!(path.is_file());
}
#[tokio::test]
async fn cancellation_before_validation_creates_no_artifact() {
    let (_fixture, shared, job, _, path) = setup();
    assert_eq!(
        complete(&shared, &job, response(), &AtomicBool::new(true))
            .await
            .unwrap_err()
            .code,
        ErrorCode::Cancelled
    );
    let (_, state) = decode(
        &shared
            .lock()
            .unwrap()
            .store
            .load_job(&job)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.phase, JobPhase::Cancelled);
    assert!(state.artifact.is_none());
    assert!(!path.parent().unwrap().exists());
}

// Metadata-only journal fixtures; no claim of an actual file with this hash.
pub(crate) fn metadata_timing() -> sungeki_project::LipSyncAnalysis {
    let wav = response().verify().unwrap();
    let mut timing =
        sungeki_media::analyze_lipsync(&wav.audio, Default::default(), &AtomicBool::new(false))
            .unwrap();
    timing.source_sha256 = "a".repeat(64);
    timing
}

#[tokio::test]
async fn successful_job_cannot_lose_its_analysis() {
    let (_fixture, shared, job, _sid, _path) = setup();
    complete(&shared, &job, response(), &AtomicBool::new(false))
        .await
        .unwrap();
    let sessions = shared.lock().unwrap();
    let (_, mut state) = decode(&sessions.store.load_job(&job).unwrap().unwrap()).unwrap();
    state.validate().unwrap();
    state.artifact.as_mut().unwrap().lip_sync = None;
    assert!(state.validate().is_err());
}
