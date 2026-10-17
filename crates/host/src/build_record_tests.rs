use crate::{
    build_record::decode,
    build_state::{Input, Output, Receipt, State, invalid},
    sessions::Sessions,
};
use sungeki_project::JobPhase;
use sungeki_storage::{JobKind, StoredJob, StoredSession};
fn input() -> Input {
    let recipe: sungeki_project::BuildRecipe = serde_json::from_str(include_str!(
        "../../../tests/fixtures/recipes/build-dialogue.json"
    ))
    .unwrap();
    Input::new(recipe, &std::env::temp_dir().join("build-fixture.vvm.json")).unwrap()
}
fn state(input: &Input, phase: JobPhase) -> State {
    let completed = if phase == JobPhase::Succeeded {
        10
    } else if phase == JobPhase::Queued {
        0
    } else {
        2
    };
    State {
        rendered: (phase == JobPhase::Succeeded).then(|| sungeki_protocol::RenderOutput {
            path: std::env::temp_dir()
                .join("output.mp4")
                .to_string_lossy()
                .into(),
            sha256: "c".repeat(64),
            byte_length: 10,
            frame_count: 30,
            font_sha256: "d".repeat(64),
            layout_version: 8,
        }),
        saved: (phase == JobPhase::Succeeded).then(|| crate::build_state::SavedProject {
            revision: 5,
            content_hash: "a".repeat(64),
            stamp: sungeki_storage::FileStamp {
                device: 1,
                inode: 1,
                hash: "b".repeat(64),
            },
        }),
        bake: (phase == JobPhase::Succeeded)
            .then_some(crate::build_state::BakeOutcome::NotRequired),
        preflight: (phase == JobPhase::Succeeded).then(|| crate::build_state::Preflight {
            revision: 5,
            content_hash: "a".repeat(64),
            frame_count: 30,
            checked_sources: 1,
        }),
        phase,
        progress: None,
        receipts: input.step_ids[..completed]
            .iter()
            .enumerate()
            .map(|(i, id)| Receipt {
                id: id.clone(),
                revision: i.min(5) as u32,
            })
            .collect(),
        child_id: matches!(
            phase,
            JobPhase::Running | JobPhase::Cancelling | JobPhase::Interrupted
        )
        .then(|| input.step_ids[2].clone()),
        expected_revision: completed.saturating_sub(1).min(5) as u32,
        output: (phase == JobPhase::Succeeded).then(|| Output {
            project_path: input.document_path.clone(),
            project_sha256: "b".repeat(64),
            render_job_id: input.step_ids[8].clone(),
            review_job_id: input.step_ids[9].clone(),
            review: sungeki_protocol::ReviewReport {
                representative_frames: None,
                report_id: input.step_ids[9].clone(),
                source_revision: 5,
                content_hash: "a".repeat(64),
                asset_fingerprint: None,
                diagnostics: sungeki_review::review_structure(&sungeki_project::Project::empty(
                    input.plan.project_id.clone(),
                    "fixture".into(),
                )),
            },
        }),
        error: matches!(phase, JobPhase::Failed | JobPhase::Interrupted)
            .then(|| invalid("test failure")),
    }
}
fn job(input: &Input, phase: JobPhase) -> StoredJob {
    StoredJob {
        id: uuid::Uuid::new_v4().to_string(),
        kind: JobKind::BuildVideo,
        session_id: "session".into(),
        version: 0,
        input: serde_json::to_vec(input).unwrap(),
        state: serde_json::to_vec(&state(input, phase)).unwrap(),
    }
}
#[test]
fn parent_recovery_keeps_receipts_and_children_without_dispatch() {
    let root = std::env::temp_dir().join(format!("vvm-parent-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("db");
    let mut sessions = Sessions::open(&path).unwrap();
    sessions
        .store
        .commit(
            None,
            &StoredSession {
                id: "session".into(),
                revision: 0,
                version: 0,
                snapshot: b"unchanged".to_vec().into(),
            },
        )
        .unwrap();
    let input = input();
    let rows: Vec<_> = [
        JobPhase::Queued,
        JobPhase::Running,
        JobPhase::Cancelling,
        JobPhase::Succeeded,
        JobPhase::Failed,
        JobPhase::Cancelled,
        JobPhase::Interrupted,
    ]
    .into_iter()
    .map(|phase| {
        let row = job(&input, phase);
        decode(&row).unwrap();
        sessions.store.create_job(&row).unwrap();
        row
    })
    .collect();
    drop(sessions);
    let recovered = Sessions::open(&path).unwrap();
    let mut restored = Vec::new();
    for row in &rows {
        let next = recovered.store.load_job(&row.id).unwrap().unwrap();
        let (_, before) = decode(row).unwrap();
        let (_, after) = decode(&next).unwrap();
        assert_eq!(next.input, row.input);
        assert_eq!(
            serde_json::to_value(&after.receipts).unwrap(),
            serde_json::to_value(&before.receipts).unwrap()
        );
        assert_eq!(after.child_id, before.child_id);
        if before.phase.is_terminal() {
            assert_eq!(next, *row);
        } else {
            assert_eq!(after.phase, JobPhase::Interrupted);
            assert_eq!(next.version, 1);
            assert!(
                after
                    .error
                    .unwrap()
                    .message
                    .contains("自動再実行はしていません")
            );
        }
        restored.push(next);
    }
    assert_eq!(
        recovered
            .store
            .list_jobs(None, None, None, 100)
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        recovered
            .store
            .load("session")
            .unwrap()
            .unwrap()
            .snapshot
            .as_slice(),
        b"unchanged"
    );
    drop(recovered);
    let again = Sessions::open(&path).unwrap();
    for row in restored {
        assert_eq!(again.store.load_job(&row.id).unwrap().unwrap(), row);
    }
    drop(again);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn impossible_progress_and_cross_step_results_are_rejected() {
    let input = input();
    let running = job(&input, JobPhase::Running);
    let original: serde_json::Value = serde_json::from_slice(&running.state).unwrap();
    for (field, value) in [
        ("expected_revision", serde_json::json!(99)),
        ("child_id", serde_json::json!(input.step_ids[9])),
        ("phase", serde_json::json!("succeeded")),
        ("phase", serde_json::json!("cancelled")),
        ("phase", serde_json::json!("queued")),
    ] {
        let mut s = original.clone();
        s[field] = value;
        assert!(
            decode(&StoredJob {
                state: serde_json::to_vec(&s).unwrap(),
                ..running.clone()
            })
            .is_err()
        );
    }
    let mut s = original.clone();
    s["receipts"][0]["id"] = serde_json::json!(input.step_ids[1]);
    assert!(
        decode(&StoredJob {
            state: serde_json::to_vec(&s).unwrap(),
            ..running.clone()
        })
        .is_err()
    );
    let mut i = input.clone();
    i.step_ids[1] = i.step_ids[0].clone();
    assert!(
        decode(&StoredJob {
            input: serde_json::to_vec(&i).unwrap(),
            ..running.clone()
        })
        .is_err()
    );
    let mut complete = state(&input, JobPhase::Succeeded);
    complete.rendered.as_mut().unwrap().frame_count += 1;
    assert!(complete.validate(&input).is_err());
    complete.rendered.as_mut().unwrap().frame_count -= 1;
    let rendered = complete.rendered.take();
    assert!(complete.validate(&input).is_err());
    complete.rendered = rendered;
    complete.output.as_mut().unwrap().review_job_id = input.step_ids[8].clone();
    assert!(complete.validate(&input).is_err());
    assert!(
        decode(&StoredJob {
            kind: JobKind::Render,
            ..running
        })
        .is_err()
    );
}

#[test]
fn changed_recipe_or_speech_count_cannot_reuse_a_fixed_plan() {
    let original = input();
    let state = state(&original, JobPhase::Running);
    let mut changed = original.clone();
    changed.recipe.scenes[0].lines[0].subtitle_text = "差し替え".into();
    assert!(state.validate(&changed).is_err());
    let mut changed = original;
    changed.speech_count = 3;
    changed.step_ids.push(uuid::Uuid::new_v4().to_string());
    assert!(state.validate(&changed).is_err());
}

#[path = "build_api_tests.rs"]
mod api_tests;
#[path = "build_bake_tests.rs"]
mod bake_tests;
#[path = "build_render_tests.rs"]
mod build_render_tests;
#[path = "build_review_tests.rs"]
mod build_review_tests;
#[path = "build_cancel_monitor_tests.rs"]
mod cancel_monitor_tests;
#[path = "build_cancel_tests.rs"]
mod cancel_tests;
#[path = "build_compose_tests.rs"]
mod compose_tests;
#[path = "build_create_tests.rs"]
mod create_tests;
#[path = "build_dispatch_tests.rs"]
mod dispatch_tests;
#[path = "build_engine_tests.rs"]
mod engine_tests;
#[path = "build_layout_apply_tests.rs"]
mod layout_apply_tests;
#[path = "build_preflight_tests.rs"]
mod preflight_tests;
#[path = "build_prepare_tests.rs"]
mod prepare_tests;
#[path = "build_render_progress_tests.rs"]
mod render_progress_tests;
#[path = "build_render_result_tests.rs"]
mod render_result_tests;
#[path = "build_reservation_tests.rs"]
mod reservation_tests;
#[path = "build_review_result_tests.rs"]
mod review_result_tests;
#[path = "build_runtime_tests.rs"]
mod runtime_tests;
#[path = "build_save_tests.rs"]
mod save_tests;
#[path = "build_settle_tests.rs"]
mod settle_tests;
#[path = "build_shutdown_tests.rs"]
mod shutdown_tests;
#[path = "build_sources_tests.rs"]
mod sources_tests;
#[path = "build_speech_apply_tests.rs"]
mod speech_apply_tests;
#[path = "build_speech_input_tests.rs"]
mod speech_input_tests;
#[path = "build_wait_tests.rs"]
mod wait_tests;

#[path = "build_speech_reuse_tests.rs"]
mod speech_reuse_tests;

#[path = "build_reuse_settlement_tests.rs"]
mod reuse_settlement_tests;

#[path = "build_continuation_tests.rs"]
mod continuation_tests;

#[path = "build_reused_original_tests.rs"]
mod reused_original_tests;

#[path = "build_reused_child_tests.rs"]
mod reused_child_tests;

#[path = "build_reuse_runtime_tests.rs"]
mod reuse_runtime_tests;

#[path = "build_api_reuse_tests.rs"]
mod api_reuse_tests;

#[path = "build_reuse_boundary_tests.rs"]
mod reuse_boundary_tests;

#[path = "build_reused_recovery_tests.rs"]
mod reused_recovery_tests;

#[path = "build_reuse_reconcile_tests.rs"]
mod reuse_reconcile_tests;

#[path = "build_reuse_source_tests.rs"]
mod reuse_source_tests;
