use super::*;
use crate::build_record as build;
use std::{sync::atomic::Ordering, time::Duration};
use sungeki_protocol::{
    BuildReuseRequest, BuildVideoRequest, SessionResponse, StartBuildVideoArgs,
};

#[tokio::test]
async fn public_start_reuses_a_successful_line_and_replay_keeps_frozen_mapping() {
    let (root, shared, parent, child, target) = super::speech_reuse_tests::setup().await;
    let mut recipe = {
        let sessions = shared.lock().unwrap();
        decode(&sessions.store.load_job(&parent).unwrap().unwrap())
            .unwrap()
            .0
            .recipe
    };
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    recipe.scenes.truncate(1);
    recipe.scenes[0].lines.truncate(1);
    recipe.scenes[0].diagrams.clear();
    let original = target.voicevox().unwrap();
    let args = StartBuildVideoArgs {
        wait_ms: None,
        job_id: uuid::Uuid::new_v4().to_string(),
        recipe,
        project_path: root
            .join("MCP受付・音声再利用.vvm.json")
            .to_string_lossy()
            .into(),
        base_url: original.endpoint().into(),
        engine_version: original.identity().engine_version.clone(),
        managed_profiles: Default::default(),
        reuse_from: Some(BuildReuseRequest {
            parent_job_id: parent.clone(),
            speech: [("0".into(), 0)].into(),
        }),
    };
    // Invalid explicit reuse must not leave a new parent/session behind.
    for mapping in [
        std::collections::BTreeMap::new(),
        [("1".into(), 0)].into(),
        [("0".into(), 199)].into(),
        [("00".into(), 0)].into(),
    ] {
        let mut invalid = args.clone();
        invalid.reuse_from.as_mut().unwrap().speech = mapping;
        let error =
            build::execute(&shared, BuildVideoRequest::Start { args: invalid }).unwrap_err();
        assert_eq!(
            error.details.get("job_created").map(String::as_str),
            Some("false")
        );
        assert!(
            shared
                .lock()
                .unwrap()
                .store
                .load_job(&args.job_id)
                .unwrap()
                .is_none()
        );
    }
    let SessionResponse::BuildVideoJob { job: first } =
        build::execute(&shared, BuildVideoRequest::Start { args: args.clone() }).unwrap()
    else {
        panic!()
    };
    let SessionResponse::BuildVideoJob { job: replay } =
        build::execute(&shared, BuildVideoRequest::Start { args: args.clone() }).unwrap()
    else {
        panic!()
    };
    assert_eq!(first.job_id, replay.job_id);
    assert_eq!(first.session_id, replay.session_id);
    let mut changed = args.clone();
    changed
        .reuse_from
        .as_mut()
        .unwrap()
        .speech
        .insert("0".into(), 1);
    assert!(build::execute(&shared, BuildVideoRequest::Start { args: changed }).is_err());
    let runtime = shared.lock().unwrap().build_runtime.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let sessions = shared.lock().unwrap();
    let row = sessions.store.load_job(&first.job_id).unwrap().unwrap();
    let (plan, state) = decode(&row).unwrap();
    assert!(state.receipts.len() >= 3, "{:?}", state.error);
    let reused = sessions.store.load_job(&plan.step_ids[2]).unwrap().unwrap();
    let (_, result) = crate::speech_journal::decode_input(&reused).unwrap();
    assert_eq!(result.execution, crate::speech_state::Execution::Reused);
    assert!(matches!(
        result.application,
        crate::speech_state::Application::Applied { .. }
    ));
    assert_ne!(reused.id, child);
    assert_ne!(
        row.session_id,
        sessions
            .store
            .load_job(&parent)
            .unwrap()
            .unwrap()
            .session_id
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
