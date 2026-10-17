use super::*;
use crate::{build_record::speech_reuse::verify, speech_journal::Input as SpeechInput};
use std::sync::{Arc, Mutex};
use sungeki_project::{ClipContent, Voice};

pub(super) async fn setup() -> (
    std::path::PathBuf,
    Arc<Mutex<Sessions>>,
    String,
    String,
    SpeechInput,
) {
    let (root, shared, parent, child) = super::speech_apply_tests::ready().await;
    crate::build_record::apply_speech(&shared, &parent, &child)
        .await
        .unwrap();
    let target = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent).unwrap().unwrap();
        let (plan, mut state) = decode(&row).unwrap();
        state.phase = JobPhase::Interrupted;
        state.error = Some(invalid("次の発話で停止したfixture"));
        state.validate(&plan).unwrap();
        sessions
            .store
            .update_job(
                &parent,
                JobKind::BuildVideo,
                row.version,
                &serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
        let child = sessions.store.load_job(&child).unwrap().unwrap();
        let (old, _) = crate::speech_journal::decode(&child).unwrap();
        let mut project = old.project().clone();
        project.id = uuid::Uuid::new_v4().to_string();
        let clip = project
            .clips
            .iter_mut()
            .find(|c| c.id == old.clip_id())
            .unwrap();
        clip.id = uuid::Uuid::new_v4().to_string();
        let clip_id = clip.id.clone();
        SpeechInput::Voicevox(
            sungeki_speech::SpeechJobInput::new(
                1,
                &project,
                &clip_id,
                old.endpoint(),
                old.identity(),
            )
            .unwrap(),
        )
    };
    (root, shared, parent, child, target)
}

#[tokio::test]
async fn reuse_checks_applied_receipts_and_returns_owned_bytes_across_project_ids_and_restart() {
    let (root, shared, parent, child, target) = setup().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load("session")
        .unwrap()
        .unwrap();
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    assert_eq!(proof.target_input_hash, target.input_hash());
    assert_eq!(proof.parent.id, parent);
    assert_eq!(proof.child.id, child);
    assert_eq!(proof.audio.info().content_hash, proof.artifact.sha256);
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load("session")
            .unwrap()
            .unwrap(),
        before
    );
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    let restarted = verify(&shared, &parent, &child, &target).await.unwrap();
    assert_eq!(restarted.parent, proof.parent);
    assert_eq!(restarted.child, proof.child);
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load("session")
            .unwrap()
            .unwrap(),
        before
    );
    std::fs::remove_file(&proof.artifact.path).unwrap();
    assert_eq!(proof.audio.info().content_hash, proof.artifact.sha256);
    assert!(verify(&shared, &parent, &child, &target).await.is_err());
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn changed_text_settings_or_engine_never_reuses_successful_speech() {
    let (root, shared, parent, child, target) = setup().await;
    let old = target.voicevox().unwrap();
    for mode in 0..3 {
        let mut project = old.project().clone();
        let mut identity = old.identity().clone();
        match mode {
            0 => {
                let clip = project
                    .clips
                    .iter_mut()
                    .find(|c| c.id == old.clip_id())
                    .unwrap();
                let ClipContent::Speech { spoken_text, .. } = &mut clip.content else {
                    panic!()
                };
                *spoken_text = Some("別の読み方です".into());
            }
            1 => {
                let Voice::Voicevox { settings, .. } = &mut project.characters[0].voice else {
                    panic!()
                };
                settings.speed_scale = 1.2;
            }
            _ => identity.engine_version = "different-version".into(),
        }
        let input = SpeechInput::Voicevox(
            sungeki_speech::SpeechJobInput::new(
                1,
                &project,
                old.clip_id(),
                old.endpoint(),
                &identity,
            )
            .unwrap(),
        );
        assert!(verify(&shared, &parent, &child, &input).await.is_err());
    }
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn changed_original_or_running_parent_is_not_a_reusable_proof() {
    let (root, shared, parent, child, target) = setup().await;
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    let mut bytes = std::fs::read(&proof.artifact.path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&proof.artifact.path, bytes).unwrap();
    assert!(verify(&shared, &parent, &child, &target).await.is_err());
    {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&parent).unwrap().unwrap();
        let (plan, mut state) = decode(&row).unwrap();
        state.phase = JobPhase::Running;
        state.error = None;
        state.validate(&plan).unwrap();
        sessions
            .store
            .update_job(
                &parent,
                JobKind::BuildVideo,
                row.version,
                &serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
    }
    let error = verify(&shared, &parent, &child, &target)
        .await
        .err()
        .unwrap();
    assert!(error.message.contains("所有・終了"));
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn pending_application_and_mismatched_parent_receipt_are_rejected_without_writes() {
    for pending in [true, false] {
        let (root, shared, parent, child, target) = setup().await;
        {
            let mut sessions = shared.lock().unwrap();
            if pending {
                let row = sessions.store.load_job(&child).unwrap().unwrap();
                let (_, mut state) = crate::speech_journal::decode_input(&row).unwrap();
                state.application = crate::speech_state::Application::PendingConflict;
                state.validate().unwrap();
                sessions
                    .store
                    .update_job(
                        &child,
                        JobKind::Speech,
                        row.version,
                        &serde_json::to_vec(&state).unwrap(),
                    )
                    .unwrap();
            } else {
                let row = sessions.store.load_job(&parent).unwrap().unwrap();
                let (plan, mut state) = decode(&row).unwrap();
                state.receipts.last_mut().unwrap().revision += 1;
                state.expected_revision += 1;
                state.validate(&plan).unwrap();
                sessions
                    .store
                    .update_job(
                        &parent,
                        JobKind::BuildVideo,
                        row.version,
                        &serde_json::to_vec(&state).unwrap(),
                    )
                    .unwrap();
            }
        }
        let before = {
            let sessions = shared.lock().unwrap();
            (
                sessions.store.load_job(&parent).unwrap(),
                sessions.store.load_job(&child).unwrap(),
                sessions.store.load("session").unwrap(),
            )
        };
        assert!(verify(&shared, &parent, &child, &target).await.is_err());
        let after = {
            let sessions = shared.lock().unwrap();
            (
                sessions.store.load_job(&parent).unwrap(),
                sessions.store.load_job(&child).unwrap(),
                sessions.store.load("session").unwrap(),
            )
        };
        assert_eq!(before, after);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}
