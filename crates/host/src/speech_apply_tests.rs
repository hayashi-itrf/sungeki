use crate::{
    sessions::Sessions,
    speech_dispatch::Reservation,
    speech_journal::{create, decode},
    speech_journal_tests::{Fixture, id, seed},
    speech_output::complete,
    speech_output_tests::response,
    speech_state::Application,
};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_editor::{Command, EditAction, EditRequest};
use sungeki_project::{Scene, TimeUs};
use sungeki_protocol::{SessionRequest, SessionResponse};

fn edit(sessions: &mut Sessions, sid: &str, commands: Vec<Command>) {
    let revision = sessions.load(sid).unwrap().0.revision;
    sessions
        .request(SessionRequest::Edit {
            session_id: sid.into(),
            edit: EditRequest {
                operation_id: id(),
                expected_revision: revision,
                action: EditAction::Apply { commands },
            },
        })
        .unwrap();
}
fn setup(scene_collision: bool) -> (Fixture, Arc<Mutex<Sessions>>, String, String) {
    let fixture = Fixture::new();
    let mut sessions = fixture.open();
    let (sid, mut input) = seed(&mut sessions);
    if scene_collision {
        let mut project = input.project().clone();
        let scene_id = id();
        project.clips[0].start_us = TimeUs(100);
        project.clips[0].duration_us = TimeUs(200);
        project.clips[0].scene_id = Some(scene_id.clone());
        project.scenes = vec![
            Scene {
                fade: None,
                id: scene_id,
                name: "導入".into(),
                start_us: TimeUs(0),
                duration_us: TimeUs(600),
            },
            Scene {
                fade: None,
                id: id(),
                name: "次".into(),
                start_us: TimeUs(1000),
                duration_us: TimeUs(1000),
            },
        ];
        edit(
            &mut sessions,
            &sid,
            vec![Command::ReplaceProject {
                project: project.clone(),
                checkpoint_id: None,
            }],
        );
        input = sungeki_speech::SpeechJobInput::new(
            2,
            &project,
            input.clip_id(),
            input.endpoint(),
            input.identity(),
        )
        .unwrap();
    }
    let job = create(&mut sessions, id(), sid.clone(), &input).unwrap();
    Reservation::acquire(&mut sessions, &job.id)
        .unwrap()
        .begin(&mut sessions)
        .unwrap()
        .confirm_finished(&mut sessions)
        .unwrap();
    (fixture, Arc::new(Mutex::new(sessions)), job.id, sid)
}
#[tokio::test]
async fn unrelated_edit_survives_application_and_single_undo_restores_before_application() {
    let (fixture, shared, job, sid) = setup(false);
    let before = {
        let mut sessions = shared.lock().unwrap();
        edit(
            &mut sessions,
            &sid,
            vec![Command::RenameProject {
                title: "合成中の無関係な編集".into(),
            }],
        );
        sessions.load(&sid).unwrap().1.history().project().clone()
    };
    complete(&shared, &job, response(), &AtomicBool::new(false))
        .await
        .unwrap();
    {
        let mut sessions = shared.lock().unwrap();
        let (_, applied) = sessions.load(&sid).unwrap();
        assert_eq!(applied.history().project().title, before.title);
        assert_eq!(applied.history().revision(), 3);
        let operation = applied.history().undo_operation_id().unwrap().to_owned();
        let result = sessions
            .request(SessionRequest::Edit {
                session_id: sid.clone(),
                edit: EditRequest {
                    operation_id: id(),
                    expected_revision: 3,
                    action: EditAction::Undo {
                        expected_operation_id: operation,
                    },
                },
            })
            .unwrap();
        let SessionResponse::Edited { snapshot, .. } = result else {
            panic!()
        };
        assert_eq!(snapshot.project, before);
        // Applied records describe the historical commit; Undo does not rerun synthesis.
        assert!(matches!(
            decode(&sessions.store.load_job(&job).unwrap().unwrap())
                .unwrap()
                .1
                .application,
            Application::Applied { revision: 3, .. }
        ));
    }
    drop(shared);
    assert_eq!(
        fixture.open().load(&sid).unwrap().1.history().project(),
        &before
    );
}
#[tokio::test]
async fn changed_target_or_scene_collision_preserves_original_as_pending() {
    for collision in [false, true] {
        let (_fixture, shared, job, sid) = setup(collision);
        let before = {
            let mut sessions = shared.lock().unwrap();
            if !collision {
                let clip = sessions.load(&sid).unwrap().1.history().project().clips[0]
                    .id
                    .clone();
                edit(
                    &mut sessions,
                    &sid,
                    vec![Command::DeleteClip { clip_id: clip }],
                );
            }
            sessions.load(&sid).unwrap().1.history().project().clone()
        };
        complete(&shared, &job, response(), &AtomicBool::new(false))
            .await
            .unwrap();
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.load(&sid).unwrap().1.history().project(), &before);
        let state = decode(&sessions.store.load_job(&job).unwrap().unwrap())
            .unwrap()
            .1;
        assert!(matches!(state.application, Application::PendingConflict));
        assert_eq!(
            std::fs::read(state.artifact.unwrap().path).unwrap(),
            response().unverified_wav
        );
    }
}
#[tokio::test]
async fn either_database_write_failure_publishes_no_edit_and_changes_neither_record() {
    for table in ["sessions", "jobs"] {
        let (fixture, shared, job, sid) = setup(false);
        let (before, cursor, old_job) = {
            let sessions = shared.lock().unwrap();
            (
                sessions.store.load(&sid).unwrap().unwrap(),
                sessions.events.cursor(),
                sessions.store.load_job(&job).unwrap().unwrap(),
            )
        };
        let fault = rusqlite::Connection::open(fixture.0.join("recovery.sqlite")).unwrap();
        fault.execute_batch(&format!("CREATE TRIGGER fail_apply BEFORE UPDATE ON {table} BEGIN SELECT RAISE(ABORT, 'apply fault'); END;")).unwrap();
        assert!(
            complete(&shared, &job, response(), &AtomicBool::new(false))
                .await
                .is_err()
        );
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.store.load(&sid).unwrap().unwrap(), before);
        assert_eq!(sessions.store.load_job(&job).unwrap().unwrap(), old_job);
        assert_eq!(sessions.events.cursor(), cursor);
    }
}

#[tokio::test]
async fn overlap_is_reported_without_moving_following_speech() {
    let (_fixture, shared, job, sid) = setup(false);
    let other_id = id();
    {
        let mut sessions = shared.lock().unwrap();
        let clip_id = sessions.load(&sid).unwrap().1.history().project().clips[0]
            .id
            .clone();
        edit(
            &mut sessions,
            &sid,
            vec![Command::DuplicateClip {
                clip_id,
                new_clip_id: other_id.clone(),
                start_us: TimeUs(500),
            }],
        );
    }
    complete(&shared, &job, response(), &AtomicBool::new(false))
        .await
        .unwrap();
    let sessions = shared.lock().unwrap();
    let project = sessions.load(&sid).unwrap().1.history().project().clone();
    assert_eq!(project.clips[1].start_us, TimeUs(500));
    assert_eq!(project.clips[1].duration_us, TimeUs(1_000_000));
    let state = decode(&sessions.store.load_job(&job).unwrap().unwrap())
        .unwrap()
        .1;
    let Application::Applied {
        overlapping_clip_ids,
        ..
    } = state.application
    else {
        panic!()
    };
    assert_eq!(overlapping_clip_ids, vec![other_id]);
}

#[test]
fn oversized_application_diagnostics_are_rejected_before_storage() {
    use crate::speech_state::{Artifact, Execution, State};
    let state = State {
        phase: sungeki_project::JobPhase::Succeeded,
        progress: None,
        execution: Execution::Finished,
        error: None,
        artifact: Some(Artifact {
            path: "/original.wav".into(),
            sha256: "a".repeat(64),
            duration_us: 1000,
            sample_rate: 24000,
            channels: 1,
            frames: 24,
            byte_len: 92,
            lip_sync: Some(crate::speech_output_tests::metadata_timing()),
        }),
        application: Application::Applied {
            revision: 1,
            overlapping_clip_ids: (1..30_000)
                .map(|n| uuid::Uuid::from_u128(n).to_string())
                .collect(),
        },
    };
    state.validate().unwrap();
    assert!(crate::speech_journal::encode(&state).is_err());
}
