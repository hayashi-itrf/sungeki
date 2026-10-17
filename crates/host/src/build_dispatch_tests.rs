use super::*;
use crate::build_record::{Advance, advance};
use std::sync::{Arc, Mutex, atomic::Ordering};

#[tokio::test]
async fn dispatcher_advances_new_project_to_speech_without_claiming_engine_success() {
    let (root, mut sessions, fixture) = super::reservation_tests::setup(false);
    let mut recipe = decode(&fixture).unwrap().0.recipe;
    for role in &mut recipe.roles {
        role.preset_id = "image-custom-v1".into();
    }
    let parent = crate::build_record::create(
        &mut sessions,
        &uuid::Uuid::new_v4().to_string(),
        recipe,
        &root.join("工程順序.vvm.json"),
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    for _ in 0..2 {
        assert_eq!(
            advance(&shared, &parent.id, None).await.unwrap(),
            Advance::Progress
        );
    }
    let before = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    assert_eq!(decode(&before).unwrap().1.receipts.len(), 2);
    assert!(advance(&shared, &parent.id, None).await.is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        before
    );
    let cancelled =
        crate::build_record::request_cancel(&mut shared.lock().unwrap(), &before).unwrap();
    assert_eq!(
        advance(&shared, &parent.id, None).await.unwrap(),
        Advance::Stopped
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .store
            .load_job(&parent.id)
            .unwrap()
            .unwrap(),
        cancelled
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn dispatcher_accepts_speech_then_runs_layout_validation_bake_and_save() {
    let (root, shared, parent_id, _) = super::speech_apply_tests::ready().await;
    assert_eq!(
        advance(&shared, &parent_id, None).await.unwrap(),
        Advance::Progress
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .speech_runtime
            .active
            .load(Ordering::SeqCst),
        0
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
    let (root, shared, parent) = super::layout_apply_tests::ready().await;
    for _ in 0..4 {
        assert_eq!(
            advance(&shared, &parent.id, None).await.unwrap(),
            Advance::Progress
        );
    }
    let sessions = shared.lock().unwrap();
    let (input, state) = decode(&sessions.store.load_job(&parent.id).unwrap().unwrap()).unwrap();
    assert_eq!(state.receipts.len(), usize::from(input.speech_count) + 6);
    assert!(state.saved.is_some());
    assert!(state.rendered.is_none());
    assert!(input.document_path.is_file());
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn reserved_missing_child_is_not_restarted_and_queued_render_is_only_observed() {
    let (root, sessions, parent, _, _) = super::speech_input_tests::setup();
    let shared = Arc::new(Mutex::new(sessions));
    assert!(advance(&shared, &parent.id, None).await.is_err());
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .speech_runtime
            .active
            .load(Ordering::SeqCst),
        0
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
    let (root, shared, parent, child) = super::build_render_tests::ready().await;
    assert_eq!(
        advance(&shared, &parent.id, None).await.unwrap(),
        Advance::Waiting
    );
    let sessions = shared.lock().unwrap();
    assert_eq!(sessions.render_runtime.active.load(Ordering::SeqCst), 0);
    assert_eq!(sessions.store.load_job(&child.id).unwrap().unwrap(), child);
    assert_eq!(
        sessions.store.load_job(&parent.id).unwrap().unwrap(),
        parent
    );
    drop(sessions);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn dispatcher_accepts_render_starts_review_and_reaches_terminal_result() {
    let (root, shared, parent, _) = super::render_result_tests::ready().await;
    assert_eq!(
        advance(&shared, &parent.id, None).await.unwrap(),
        Advance::Progress
    );
    assert_eq!(
        advance(&shared, &parent.id, None).await.unwrap(),
        Advance::Waiting
    );
    let runtime = shared.lock().unwrap().review_runtime.clone();
    super::build_review_tests::idle(&runtime).await;
    assert_eq!(
        advance(&shared, &parent.id, None).await.unwrap(),
        Advance::Progress
    );
    assert_eq!(
        advance(&shared, &parent.id, None).await.unwrap(),
        Advance::Stopped
    );
    let row = shared
        .lock()
        .unwrap()
        .store
        .load_job(&parent.id)
        .unwrap()
        .unwrap();
    let (_, state) = decode(&row).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert!(state.output.unwrap().review.diagnostics.summary.not_run > 0);
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .render_runtime
            .active
            .load(Ordering::SeqCst),
        0
    );
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn child_failure_reports_reason_without_advancing_or_restarting() {
    use sungeki_protocol::{ApiError, ErrorCode};
    let (root, shared, parent, mut child) = super::build_render_tests::ready().await;
    for phase in [
        JobPhase::Failed,
        JobPhase::Cancelled,
        JobPhase::Interrupted,
        JobPhase::Cancelling,
    ] {
        let mut reason = ApiError::new(ErrorCode::ValidationFailed, "文字が枠からはみ出しています");
        reason.details.insert("clip_id".into(), "subtitle-1".into());
        reason.retryable = true;
        reason
            .suggested_actions
            .push("文字サイズを見直してください".into());
        let state = crate::render_record::State {
            chunk_sha256: Vec::new(),
            phase,
            progress: None,
            output: None,
            error: (phase == JobPhase::Failed).then_some(reason),
        };
        {
            let mut sessions = shared.lock().unwrap();
            crate::render_record::save(&mut sessions, &child, &state).unwrap();
            child = sessions.store.load_job(&child.id).unwrap().unwrap();
        }
        let error = advance(&shared, &parent.id, None).await.unwrap_err();
        assert!(error.message.starts_with("動画出力で制作が停止しました："));
        assert_eq!(error.details["build_child_job_id"], child.id);
        assert_eq!(error.details["build_child_phase"], format!("{phase:?}"));
        assert!(!error.retryable);
        if phase == JobPhase::Failed {
            assert_eq!(error.code, ErrorCode::ValidationFailed);
            assert!(error.message.contains("文字が枠からはみ出しています"));
            assert_eq!(error.details["clip_id"], "subtitle-1");
            assert_eq!(error.details["build_child_retryable"], "true");
            assert!(
                error
                    .suggested_actions
                    .iter()
                    .any(|s| s == "文字サイズを見直してください")
            );
        } else if phase == JobPhase::Cancelled {
            assert_eq!(error.code, ErrorCode::Cancelled);
        } else if phase == JobPhase::Interrupted {
            assert!(error.message.contains("中断"));
        } else {
            assert!(error.message.contains("停止確認がまだ完了していません"));
        }
        let sessions = shared.lock().unwrap();
        assert_eq!(
            sessions.store.load_job(&parent.id).unwrap().unwrap(),
            parent
        );
        assert_eq!(sessions.store.load_job(&child.id).unwrap().unwrap(), child);
        assert_eq!(sessions.render_runtime.active.load(Ordering::SeqCst), 0);
    }
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
