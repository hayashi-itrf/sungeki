use super::*;
use crate::build_record::{self as build, continuation::Continuation, speech_reuse::verify};
use crate::speech_journal::Input as SpeechInput;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

pub(super) async fn setup() -> (
    std::path::PathBuf,
    Arc<Mutex<Sessions>>,
    StoredJob,
    String,
    SpeechInput,
    build::speech_reuse::VerifiedReuse,
    crate::speech_state::Artifact,
) {
    let (root, shared, old_parent, old_child, preliminary) =
        super::speech_reuse_tests::setup().await;
    let proof = verify(&shared, &old_parent, &old_child, &preliminary)
        .await
        .unwrap();
    let source = Continuation::from_verified(&[(0, proof)], 2).unwrap();
    let prepared = {
        let mut sessions = shared.lock().unwrap();
        let old = sessions.store.load_job(&old_parent).unwrap().unwrap();
        let mut recipe = decode(&old).unwrap().0.recipe;
        for role in &mut recipe.roles {
            role.preset_id = "image-custom-v1".into();
        }
        let row = build::create_continued(
            &mut sessions,
            &uuid::Uuid::new_v4().to_string(),
            recipe,
            &root.join("音声を引き継ぐ作品.vvm.json"),
            None,
            Default::default(),
            Some(source.clone()),
        )
        .unwrap();
        let build::Reservation::New(row) = build::reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        row
    };
    build::prepare(&shared, &prepared).unwrap();
    let (parent, child, target, directory) = {
        let mut sessions = shared.lock().unwrap();
        let row = sessions.store.load_job(&prepared.id).unwrap().unwrap();
        let build::Reservation::New(row) = build::reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        build::compose(&mut sessions, &row).unwrap();
        let row = sessions.store.load_job(&row.id).unwrap().unwrap();
        let build::Reservation::New(row) = build::reserve(&mut sessions, &row).unwrap() else {
            panic!()
        };
        let (plan, state) = decode(&row).unwrap();
        let (_, session) = sessions.load(&row.session_id).unwrap();
        let input = build::reuse_target::prepare(
            &preliminary,
            state.expected_revision,
            session.history().project(),
            &plan.plan.script.speech_ids[0],
            &row.id,
        )
        .unwrap();
        (
            row,
            state.child_id.unwrap(),
            input,
            sessions.speech_directory.as_ref().unwrap().clone(),
        )
    };
    let proof = verify(&shared, &old_parent, &old_child, &target)
        .await
        .unwrap();
    let artifact = build::reused_original::publish(
        &directory,
        &child,
        0,
        target.input_hash(),
        &source,
        &proof,
        &AtomicBool::new(false),
    )
    .unwrap();
    (root, shared, parent, child, target, proof, artifact)
}

#[tokio::test]
async fn reuse_registers_without_dispatch_then_applies_once_and_replays_after_restart() {
    let (root, shared, parent, child, target, proof, artifact) = setup().await;
    let before = shared
        .lock()
        .unwrap()
        .store
        .load(&parent.session_id)
        .unwrap()
        .unwrap();
    let row =
        build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
            .await
            .unwrap();
    let (_, state) = crate::speech_journal::decode_input(&row).unwrap();
    assert_eq!(state.execution, crate::speech_state::Execution::Reused);
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load(&parent.session_id)
            .unwrap()
            .unwrap(),
        before
    );
    assert!(
        crate::speech_dispatch::Reservation::acquire(&mut shared.lock().unwrap(), &child).is_err()
    );
    let receipt = build::apply_speech(&shared, &parent.id, &child)
        .await
        .unwrap();
    let applied = shared
        .lock()
        .unwrap()
        .store
        .load(&parent.session_id)
        .unwrap()
        .unwrap();
    assert_eq!(applied.revision, before.revision + 1);
    assert_eq!(
        build::apply_speech(&shared, &parent.id, &child)
            .await
            .unwrap(),
        receipt
    );
    let replay =
        build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
            .await
            .unwrap();
    assert!(matches!(
        crate::speech_journal::decode_input(&replay)
            .unwrap()
            .1
            .application,
        crate::speech_state::Application::Applied { .. }
    ));
    drop(shared);
    let shared = Arc::new(Mutex::new(Sessions::open(&root.join("db")).unwrap()));
    assert_eq!(
        build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
            .await
            .unwrap(),
        replay
    );
    assert_eq!(
        build::apply_speech(&shared, &parent.id, &child)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load(&parent.session_id)
            .unwrap()
            .unwrap(),
        applied
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn cancellation_or_modified_carried_original_never_creates_a_successful_child() {
    for cancel in [true, false] {
        let (root, shared, parent, child, target, proof, artifact) = setup().await;
        if cancel {
            build::cancel_owned(&shared, &parent).unwrap();
        } else {
            let mut bytes = std::fs::read(&artifact.path).unwrap();
            *bytes.last_mut().unwrap() ^= 1;
            std::fs::write(&artifact.path, bytes).unwrap();
        }
        let before = shared
            .lock()
            .unwrap()
            .store
            .load(&parent.session_id)
            .unwrap()
            .unwrap();
        assert!(
            build::reused_child::register(&shared, &parent.id, &child, &target, &proof, &artifact)
                .await
                .is_err()
        );
        let sessions = shared.lock().unwrap();
        assert!(sessions.store.load_job(&child).unwrap().is_none());
        assert_eq!(
            sessions.store.load(&parent.session_id).unwrap().unwrap(),
            before
        );
        assert_eq!(
            sessions.store.load_job(&proof.child.id).unwrap().unwrap(),
            proof.child
        );
        drop(sessions);
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}
