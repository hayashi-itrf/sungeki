use super::*;
use crate::build_record::{Reservation, reserve};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_editor::{EditAction, EditRequest};
use sungeki_project::{ClipContent, Voice};
use sungeki_speech::{SpeechJobInput, VoicevoxIdentity};

pub(super) async fn ready() -> (std::path::PathBuf, Arc<Mutex<Sessions>>, StoredJob) {
    let (root, shared, parent, first) = super::speech_apply_tests::ready().await;
    crate::build_record::apply_speech(&shared, &parent, &first)
        .await
        .unwrap();
    let child = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent).unwrap().unwrap();
        let Reservation::New(row) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        let (input, state) = decode(&row).unwrap();
        let child = state.child_id.unwrap();
        let (_, session) = sessions.load(&row.session_id).unwrap();
        let project = session.history().project();
        let clip = &project.clips[1];
        let ClipContent::Speech { character_id, .. } = &clip.content else {
            panic!()
        };
        let character = project
            .characters
            .iter()
            .find(|c| c.id == *character_id)
            .unwrap();
        let Voice::Voicevox {
            engine_id,
            core_version,
            ..
        } = &character.voice
        else {
            panic!("VOICEVOX fixture required");
        };
        let speech = SpeechJobInput::new(
            state.expected_revision,
            project,
            &input.plan.script.speech_ids[1],
            "http://127.0.0.1:55021",
            &VoicevoxIdentity {
                engine_id: engine_id.clone(),
                core_version: core_version.clone(),
                engine_version: "1.0".into(),
            },
        )
        .unwrap()
        .for_parent(&parent)
        .unwrap();
        crate::speech_journal::create(&mut sessions, child.clone(), row.session_id, &speech)
            .unwrap();
        crate::speech_dispatch::Reservation::acquire(&mut sessions, &child)
            .unwrap()
            .begin(&mut sessions)
            .unwrap()
            .confirm_finished(&mut sessions)
            .unwrap();
        child
    };
    crate::speech_output::complete(
        &shared,
        &child,
        crate::speech_output_tests::response(),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();
    crate::build_record::apply_speech(&shared, &parent, &child)
        .await
        .unwrap();
    let row = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent).unwrap().unwrap();
        let Reservation::New(row) = reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        row
    };
    (root, shared, row)
}
#[tokio::test]
async fn two_speech_results_then_layout_commit_replay_restart_and_undo() {
    let (root, shared, row) = ready().await;
    let receipt;
    {
        let mut sessions = shared.lock().unwrap();
        let (_, before) = sessions.load("session").unwrap();
        receipt = crate::build_record::layout(&mut sessions, &row).unwrap();
        let current = sessions.store.load_job(&row.id).unwrap().unwrap();
        let (input, state) = decode(&current).unwrap();
        assert_eq!(state.receipts.len(), 5);
        assert!(state.child_id.is_none());
        assert_eq!(state.receipts[4].revision, receipt.revision);
        let (stored, mut after) = sessions.load("session").unwrap();
        assert_eq!(after.history().project().clips.len(), 16);
        for original in &before.history().project().clips {
            let clip = after
                .history()
                .project()
                .clips
                .iter()
                .find(|c| c.id == original.id)
                .unwrap();
            assert_eq!(clip.scene_id, original.scene_id);
            assert_eq!(clip.content, original.content);
            assert_eq!(clip.start_us.0, original.start_us.0 + 11_000_000);
        }
        assert_eq!(
            crate::build_record::layout(&mut sessions, &current).unwrap(),
            receipt
        );
        assert_eq!(sessions.store.load("session").unwrap().unwrap(), stored);
        after
            .execute(EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: stored.revision,
                action: EditAction::Undo {
                    expected_operation_id: input.step_ids[4].clone(),
                },
            })
            .unwrap();
        assert_eq!(after.history().project(), before.history().project());
        sessions
            .store
            .commit(
                Some(stored.version),
                &StoredSession {
                    id: stored.id,
                    version: stored.version + 1,
                    revision: after.history().revision(),
                    snapshot: after.snapshot().unwrap().into(),
                },
            )
            .unwrap();
    }
    drop(shared);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let row = sessions.store.load_job(&row.id).unwrap().unwrap();
    assert_eq!(decode(&row).unwrap().1.phase, JobPhase::Interrupted);
    let before = sessions.store.load("session").unwrap().unwrap();
    assert_eq!(
        crate::build_record::layout(&mut sessions, &row).unwrap(),
        receipt
    );
    assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn cancelled_parent_or_unapplied_child_never_adds_visuals() {
    for cancel in [false, true] {
        let (root, shared, row) = ready().await;
        {
            let mut sessions = shared.lock().unwrap();
            let before = sessions.store.load("session").unwrap().unwrap();
            let expected = if cancel {
                crate::build_record::request_cancel(&mut sessions, &row).unwrap()
            } else {
                let (input, _) = decode(&row).unwrap();
                let child = sessions
                    .store
                    .load_job(&input.step_ids[2])
                    .unwrap()
                    .unwrap();
                let (_, mut state) = crate::speech_journal::decode(&child).unwrap();
                state.application = crate::speech_state::Application::PendingConflict;
                sessions
                    .store
                    .update_job(
                        &child.id,
                        child.kind,
                        child.version,
                        &serde_json::to_vec(&state).unwrap(),
                    )
                    .unwrap();
                row.clone()
            };
            assert!(crate::build_record::layout(&mut sessions, &row).is_err());
            assert_eq!(sessions.store.load("session").unwrap().unwrap(), before);
            assert_eq!(sessions.store.load_job(&row.id).unwrap().unwrap(), expected);
        }
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}
