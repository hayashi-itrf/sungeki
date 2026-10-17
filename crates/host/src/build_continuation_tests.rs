use super::*;
use crate::build_record::{continuation::Continuation, create_continued, speech_reuse::verify};

#[tokio::test]
async fn continuation_is_atomic_idempotent_and_persists_across_restart() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    let fixed = Continuation::from_verified(&[(0, proof)], 2).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let path = root.join("続行・成功音声を再利用.vvm.json");
    let (recipe, row, source) = {
        let mut sessions = shared.lock().unwrap();
        let source = sessions.store.load_job(&parent).unwrap().unwrap();
        let recipe = decode(&source).unwrap().0.recipe;
        let row = create_continued(
            &mut sessions,
            &id,
            recipe.clone(),
            &path,
            None,
            Default::default(),
            Some(fixed.clone()),
        )
        .unwrap();
        assert_eq!(
            create_continued(
                &mut sessions,
                &id,
                recipe.clone(),
                &path,
                None,
                Default::default(),
                Some(fixed.clone())
            )
            .unwrap(),
            row
        );
        assert!(crate::build_record::create(&mut sessions, &id, recipe.clone(), &path).is_err());
        assert_ne!(row.session_id, source.session_id);
        assert!(!path.exists());
        assert_eq!(sessions.store.load_job(&parent).unwrap().unwrap(), source);
        assert_eq!(decode(&row).unwrap().0.continuation, Some(fixed.clone()));
        (recipe, row, source)
    };
    drop(shared);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let recovered = create_continued(
        &mut sessions,
        &id,
        recipe,
        &path,
        None,
        Default::default(),
        Some(fixed.clone()),
    )
    .unwrap();
    assert_eq!(recovered.input, row.input);
    assert_eq!(decode(&recovered).unwrap().1.phase, JobPhase::Interrupted);
    assert_eq!(decode(&recovered).unwrap().0.continuation, Some(fixed));
    assert_eq!(sessions.store.load_job(&parent).unwrap().unwrap(), source);
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn duplicate_targets_and_source_changes_are_rejected_before_new_parent_is_saved() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    let other = verify(&shared, &parent, &child, &target).await.unwrap();
    assert!(Continuation::from_verified(&[(0, proof), (0, other)], 2).is_err());
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    assert!(Continuation::from_verified(&[(2, proof)], 2).is_err());
    let proof = verify(&shared, &parent, &child, &target).await.unwrap();
    let fixed = Continuation::from_verified(&[(0, proof)], 2).unwrap();
    let mut sessions = shared.lock().unwrap();
    let source = sessions.store.load_job(&parent).unwrap().unwrap();
    let recipe = decode(&source).unwrap().0.recipe;
    // A new durable generation, even with the same content, invalidates the stamp.
    sessions
        .store
        .update_job(&parent, JobKind::BuildVideo, source.version, &source.state)
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    assert!(
        create_continued(
            &mut sessions,
            &id,
            recipe,
            &root.join("拒否.vvm.json"),
            None,
            Default::default(),
            Some(fixed)
        )
        .is_err()
    );
    assert!(sessions.store.load_job(&id).unwrap().is_none());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
