use super::*;

#[test]
fn creation_fixes_ids_replays_and_keeps_interrupted_state_after_restart() {
    let root = std::env::temp_dir().join(format!("vvm-build-create-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let recipe = input().recipe;
    let path = root.join("二人の解説.vvm.json");
    let id = uuid::Uuid::new_v4().to_string();
    let parent = crate::build_record::create(&mut sessions, &id, recipe.clone(), &path).unwrap();
    let (fixed, state) = decode(&parent).unwrap();
    assert_eq!(state.phase, JobPhase::Queued);
    assert!(state.receipts.is_empty());
    let before = sessions.store.load(&parent.session_id).unwrap().unwrap();
    let (_, current) = sessions.load(&parent.session_id).unwrap();
    assert_eq!(current.history().project().id, fixed.plan.project_id);
    assert_eq!(current.history().project().title, recipe.title);
    assert!(current.history().project().clips.is_empty());
    assert!(!path.exists());
    assert_eq!(
        crate::build_record::create(&mut sessions, &id, recipe.clone(), &path).unwrap(),
        parent
    );
    let mut changed = recipe.clone();
    changed.title = "別作品".into();
    assert!(crate::build_record::create(&mut sessions, &id, changed, &path).is_err());
    assert!(
        crate::build_record::create(
            &mut sessions,
            &id,
            recipe.clone(),
            &root.join("別.vvm.json")
        )
        .is_err()
    );
    assert_eq!(
        sessions.store.load(&parent.session_id).unwrap().unwrap(),
        before
    );
    drop(sessions);
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let restored = crate::build_record::create(&mut sessions, &id, recipe.clone(), &path).unwrap();
    assert_eq!(restored.input, parent.input);
    assert_eq!(decode(&restored).unwrap().1.phase, JobPhase::Interrupted);
    assert_eq!(
        sessions
            .store
            .list_jobs(Some(JobKind::BuildVideo), None, None, 100)
            .unwrap()
            .len(),
        1
    );
    let occupied = root.join("既存.vvm.json");
    std::fs::write(&occupied, b"keep").unwrap();
    let next_id = uuid::Uuid::new_v4().to_string();
    assert!(crate::build_record::create(&mut sessions, &next_id, recipe, &occupied).is_err());
    assert!(sessions.store.load_job(&next_id).unwrap().is_none());
    assert_eq!(std::fs::read(occupied).unwrap(), b"keep");
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}
