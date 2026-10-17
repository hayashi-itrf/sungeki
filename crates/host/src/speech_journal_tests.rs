use crate::{
    sessions::Sessions,
    speech_journal::{create, decode, save},
    speech_state::{Execution, State},
};
use sungeki_project::*;
use sungeki_protocol::*;
use sungeki_speech::{SpeechJobInput, VoicevoxIdentity};
pub(super) fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub(super) struct Fixture(pub(super) std::path::PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("vvm-speech-journal-{}", id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn open(&self) -> Sessions {
        Sessions::open(&self.0.join("recovery.sqlite")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(super) fn seed(sessions: &mut Sessions) -> (String, SpeechJobInput) {
    seed_at(sessions, "http://127.0.0.1:55021", "engine")
}
pub(super) fn seed_at(
    sessions: &mut Sessions,
    endpoint: &str,
    engine: &str,
) -> (String, SpeechJobInput) {
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "合成記録の検証".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let mut project = snapshot.project;
    let (character_id, track_id, clip_id) = (id(), id(), id());
    project.characters.push(Character {
        id: character_id.clone(),
        name: "検証用".into(),
        credit: String::new(),
        subtitle_style: SubtitleStyle {
            layout: None,
            outline: None,
            font_size: 64.0,
            color: "#ffffff".into(),
        },
        voice: Voice::Voicevox {
            engine_id: engine.into(),
            core_version: "1".into(),
            settings: VoicevoxSettings {
                style_id: 1,
                speed_scale: 1.0,
                pitch_scale: 0.0,
                intonation_scale: 1.0,
                volume_scale: 1.0,
                pre_phoneme_length: 0.1,
                post_phoneme_length: 0.1,
                pause_length: None,
                pause_length_scale: 1.0,
                output_sampling_rate: 24000,
                output_stereo: false,
                enable_interrogative_upspeak: true,
            },
        },
    });
    project.tracks.push(Track {
        id: track_id.clone(),
        name: "音声".into(),
        kind: TrackKind::Audio,
        visible: true,
        muted: false,
    });
    project.clips.push(Clip {
        id: clip_id.clone(),
        track_id,
        scene_id: None,
        start_us: TimeUs(0),
        duration_us: TimeUs(1_000_000),
        transform: Transform {
            motion: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            opacity: 1.0,
            fade: None,
            flip_x: None,
        },
        content: ClipContent::Speech {
            gain_db: None,
            audio: None,
            character_id,
            subtitle_text: "字幕".into(),
            spoken_text: None,
        },
    });
    sessions
        .request(SessionRequest::Edit {
            session_id: snapshot.session_id.clone(),
            edit: EditRequest {
                operation_id: id(),
                expected_revision: 0,
                action: EditAction::Apply {
                    commands: vec![sungeki_editor::Command::ReplaceProject {
                        project: project.clone(),
                        checkpoint_id: None,
                    }],
                },
            },
        })
        .unwrap();
    let input = SpeechJobInput::new(
        1,
        &project,
        &clip_id,
        endpoint,
        &VoicevoxIdentity {
            engine_id: engine.into(),
            engine_version: "1".into(),
            core_version: "1".into(),
        },
    )
    .unwrap();
    (snapshot.session_id, input)
}
#[test]
fn durable_dispatch_recovery_quarantines_engine_without_restarting_or_changing_project() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let jid = id();
    let queued = create(&mut sessions, jid.clone(), sid.clone(), &input).unwrap();
    let mut running = State::queued();
    running.phase = JobPhase::Running;
    save(&mut sessions, &queued, &running).unwrap();
    let prepared = sessions.store.load_job(&jid).unwrap().unwrap();
    running.execution = Execution::PossiblyRunning;
    save(&mut sessions, &prepared, &running).unwrap();
    assert!(save(&mut sessions, &prepared, &running).is_err()); // stale CAS
    drop(sessions);
    let mut sessions = fixture.open();
    let record = sessions.store.load_job(&jid).unwrap().unwrap();
    let (_, state) = decode(&record).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert_eq!(state.execution, Execution::PossiblyRunning);
    let pending = sessions.speech_resources.quarantined().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].owner(), jid);
    assert_eq!(
        create(&mut sessions, jid.clone(), sid.clone(), &input).unwrap(),
        record
    );
    assert_eq!(
        sessions.load(&sid).unwrap().1.history().project(),
        input.project()
    );
    sessions
        .request(SessionRequest::Edit {
            session_id: sid.clone(),
            edit: EditRequest {
                operation_id: id(),
                expected_revision: 1,
                action: EditAction::Apply {
                    commands: vec![sungeki_editor::Command::RenameProject {
                        title: "別編集後".into(),
                    }],
                },
            },
        })
        .unwrap();
    assert_eq!(
        create(&mut sessions, jid.clone(), sid.clone(), &input).unwrap(),
        record
    );
    assert_eq!(
        create(&mut sessions, id(), sid, &input).unwrap_err().code,
        ErrorCode::Conflict
    );
    let mut bad = state.clone();
    bad.phase = JobPhase::Running;
    assert!(save(&mut sessions, &record, &bad).is_err());
    drop(sessions);
    let sessions = fixture.open();
    assert_eq!(sessions.store.load_job(&jid).unwrap().unwrap(), record);
    assert_eq!(sessions.speech_resources.quarantined().unwrap().len(), 1);
}
#[test]
fn unstarted_recovery_does_not_quarantine_and_invalid_success_is_rejected() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let jid = id();
    let record = create(&mut sessions, jid.clone(), sid, &input).unwrap();
    let mut bad = State::queued();
    bad.phase = JobPhase::Succeeded;
    bad.execution = Execution::Finished;
    assert!(save(&mut sessions, &record, &bad).is_err());
    drop(sessions);
    let sessions = fixture.open();
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    assert_eq!(
        decode(&sessions.store.load_job(&jid).unwrap().unwrap())
            .unwrap()
            .1
            .phase,
        JobPhase::Interrupted
    );
}
#[test]
fn failure_does_not_clear_unknown_work_and_settlement_is_durable() {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let jid = id();
    let record = create(&mut sessions, jid.clone(), sid, &input).unwrap();
    let mut state = State::queued();
    state.phase = JobPhase::Running;
    state.execution = Execution::PossiblyRunning;
    save(&mut sessions, &record, &state).unwrap();
    let record = sessions.store.load_job(&jid).unwrap().unwrap();
    state.phase = JobPhase::Failed;
    state.error = Some(ApiError::new(ErrorCode::EngineUnavailable, "終了不明"));
    save(&mut sessions, &record, &state).unwrap();
    drop(sessions);
    let mut sessions = fixture.open();
    assert_eq!(sessions.speech_resources.quarantined().unwrap().len(), 1);
    let record = sessions.store.load_job(&jid).unwrap().unwrap();
    state.execution = Execution::Finished;
    save(&mut sessions, &record, &state).unwrap();
    drop(sessions);
    let sessions = fixture.open();
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    assert_eq!(
        decode(&sessions.store.load_job(&jid).unwrap().unwrap())
            .unwrap()
            .1
            .phase,
        JobPhase::Failed
    );
}
#[test]
fn success_requires_consistent_artifact_and_cannot_rewrite_a_completed_result() {
    use crate::speech_state::{Application, Artifact};
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, input) = seed(&mut sessions);
    let jid = id();
    let record = create(&mut sessions, jid.clone(), sid, &input).unwrap();
    let mut state = State::queued();
    state.phase = JobPhase::Running;
    state.execution = Execution::PossiblyRunning;
    save(&mut sessions, &record, &state).unwrap();
    let record = sessions.store.load_job(&jid).unwrap().unwrap();
    state.phase = JobPhase::Succeeded;
    state.execution = Execution::Finished;
    state.artifact = Some(Artifact {
        path: "/tmp/journal-metadata-only/audio.wav".into(),
        sha256: "a".repeat(64),
        duration_us: 1000,
        sample_rate: 24000,
        channels: 1,
        frames: 24,
        byte_len: 92,
        lip_sync: Some(crate::speech_output_tests::metadata_timing()),
    });
    state.application = Application::PendingConflict;
    let mut wrong = state.clone();
    wrong.artifact.as_mut().unwrap().duration_us = 999;
    assert!(save(&mut sessions, &record, &wrong).is_err());
    let mut wrong_settings = state.clone();
    wrong_settings
        .artifact
        .as_mut()
        .unwrap()
        .lip_sync
        .as_mut()
        .unwrap()
        .settings
        .min_open_us += 1;
    assert!(save(&mut sessions, &record, &wrong_settings).is_err());
    save(&mut sessions, &record, &state).unwrap();
    let record = sessions.store.load_job(&jid).unwrap().unwrap();
    state.artifact.as_mut().unwrap().sha256 = "b".repeat(64);
    assert!(save(&mut sessions, &record, &state).is_err());
    drop(sessions);
    let sessions = fixture.open();
    assert!(sessions.speech_resources.quarantined().unwrap().is_empty());
    assert!(matches!(
        decode(&sessions.store.load_job(&jid).unwrap().unwrap())
            .unwrap()
            .1
            .application,
        Application::PendingConflict
    ));
}
