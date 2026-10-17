use super::*;
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_protocol::*;
#[tokio::test]
async fn public_start_get_list_cancel_replay_keeps_one_parent_and_session() {
    let (root, sessions, fixture) = super::reservation_tests::setup(false);
    let recipe = decode(&fixture).unwrap().0.recipe;
    let shared = Arc::new(Mutex::new(sessions));
    let args = StartBuildVideoArgs {
        wait_ms: None,
        reuse_from: None,
        managed_profiles: Default::default(),
        job_id: uuid::Uuid::new_v4().to_string(),
        recipe,
        project_path: root.join("API制作.vvm.json").to_string_lossy().into(),
        base_url: "http://127.0.0.1:55021".into(),
        engine_version: "1.0".into(),
    };
    let mut invalid = args.clone();
    invalid.job_id = uuid::Uuid::new_v4().to_string();
    invalid.recipe.roles.clear();
    let rejected = crate::build_record::execute(
        &shared,
        BuildVideoRequest::Start {
            args: invalid.clone(),
        },
    )
    .unwrap_err();
    assert_eq!(rejected.details["job_created"], "false");
    assert_eq!(rejected.details["job_id"], invalid.job_id);
    let SessionResponse::BuildVideoJob { job: first } = Sessions::execute(
        &shared,
        SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start { args: args.clone() },
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(first.phase, JobPhase::Queued);
    assert_eq!(first.progress.as_ref().unwrap().stage, "制作開始待ち");
    assert_eq!(first.progress.as_ref().unwrap().fraction, None);
    let SessionResponse::BuildVideoJob { job: again } = Sessions::execute(
        &shared,
        SessionRequest::BuildVideo {
            request: BuildVideoRequest::Start { args: args.clone() },
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(first.session_id, again.session_id);
    let mut changed = args.clone();
    changed.engine_version = "2.0".into();
    assert!(
        Sessions::execute(
            &shared,
            SessionRequest::BuildVideo {
                request: BuildVideoRequest::Start { args: changed }
            }
        )
        .is_err()
    );
    let SessionResponse::BuildVideoJobs { job_ids } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::List {
            after: None,
            limit: 100,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job_ids.len(), 2); // unrelated fixture plus the new production
    assert!(job_ids.contains(&args.job_id));
    crate::build_record::execute(
        &shared,
        BuildVideoRequest::Cancel {
            job_id: args.job_id.clone(),
        },
    )
    .unwrap();
    let active = shared.lock().unwrap().build_runtime.active.clone();
    tokio::time::timeout(Duration::from_secs(3), async {
        while active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let SessionResponse::BuildVideoJob { job } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: args.job_id.clone(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Cancelled);
    assert_eq!(job.progress.as_ref().unwrap().stage, "制作を取り消しました");
    assert!(job.output.is_none());
    assert_eq!(job.completed_steps, 0);
    assert!(!job.stale);
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .speech_runtime
            .active
            .load(Ordering::SeqCst),
        0
    );
    let SessionResponse::BuildVideoJob { job } =
        crate::build_record::execute(&shared, BuildVideoRequest::Start { args }).unwrap()
    else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Cancelled);
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn running_api_progress_names_the_reserved_step_without_time_percentage() {
    let (root, mut sessions, row) = super::reservation_tests::setup(false);
    crate::build_record::reserve(&mut sessions, &row).unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    let SessionResponse::BuildVideoJob { job } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: row.id.clone(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Running);
    assert_eq!(
        job.progress.unwrap(),
        sungeki_project::Progress {
            stage: "素材とプリセットを確認中".into(),
            fraction: None
        }
    );
    assert_eq!(job.completed_steps, 0);
    let stored = shared
        .lock()
        .unwrap()
        .store
        .load_job(&row.id)
        .unwrap()
        .unwrap();
    assert!(decode(&stored).unwrap().1.progress.is_none()); // Get is read-only
    {
        let mut sessions = shared.lock().unwrap();
        let cancelling = crate::build_record::request_cancel(&mut sessions, &stored).unwrap();
        let (_, mut state) = decode(&cancelling).unwrap();
        state.progress = Some(sungeki_project::Progress {
            stage: "停止確認未完了".into(),
            fraction: None,
        });
        sessions
            .store
            .update_job(
                &row.id,
                JobKind::BuildVideo,
                cancelling.version,
                &serde_json::to_vec(&state).unwrap(),
            )
            .unwrap();
    }
    let SessionResponse::BuildVideoJob { job } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: row.id.clone(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.progress.unwrap().stage, "停止確認未完了");
    {
        let mut sessions = shared.lock().unwrap();
        let cancelling = sessions.store.load_job(&row.id).unwrap().unwrap();
        assert!(crate::build_record::settle_cancel(&mut sessions, &cancelling).unwrap());
    }
    let SessionResponse::BuildVideoJob { job } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: row.id.clone(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.progress.unwrap().stage, "制作を取り消しました");
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn completed_parent_marks_old_rendering_stale_but_unrendered_parent_does_not() {
    let input = super::input();
    let mut completed = super::state(&input, JobPhase::Succeeded);
    for version in [7, 8, sungeki_render::CURRENT_LAYOUT_VERSION] {
        completed.rendered.as_mut().unwrap().layout_version = version;
        assert_eq!(
            crate::build_record::api::has_outdated_output(&completed),
            version != sungeki_render::CURRENT_LAYOUT_VERSION
        );
    }
    for phase in [JobPhase::Queued, JobPhase::Running, JobPhase::Failed] {
        assert!(!crate::build_record::api::has_outdated_output(
            &super::state(&input, phase)
        ));
    }
}

#[test]
fn speech_progress_reports_owned_child_without_changing_durable_jobs() {
    let (root, mut sessions, parent, input, child_id) = super::speech_input_tests::setup();
    let get = |sessions: &Sessions| sessions.store.load_job(&parent.id).unwrap().unwrap();
    let child = crate::speech_journal::create(
        &mut sessions,
        child_id.clone(),
        parent.session_id.clone(),
        &input,
    )
    .unwrap();
    let shared = Arc::new(Mutex::new(sessions));
    for (stage, label) in [
        ("speech.inspect_originals", "音声モデル・原本を確認中"),
        ("speech.start_engine", "音声エンジンを起動中"),
        ("speech.recheck_originals", "音声モデル・原本を確認中"),
        ("speech.synthesize", "セリフを音声に変換中"),
        ("speech.verify_output", "生成した音声を検証中"),
        ("speech.stop_engine", "音声エンジンを停止中"),
        ("speech.save_audio", "生成した音声を保存中"),
    ] {
        let before = {
            let mut sessions = shared.lock().unwrap();
            let row = sessions.store.load_job(&child.id).unwrap().unwrap();
            let (_, mut state) = crate::speech_journal::decode_input(&row).unwrap();
            state.phase = JobPhase::Running;
            state.progress = Some(sungeki_project::Progress {
                stage: stage.into(),
                fraction: None,
            });
            crate::speech_journal::save(&mut sessions, &row, &state).unwrap();
            (
                get(&sessions),
                sessions.store.load_job(&child.id).unwrap().unwrap(),
            )
        };
        let SessionResponse::BuildVideoJob { job } = crate::build_record::execute(
            &shared,
            BuildVideoRequest::Get {
                job_id: parent.id.clone(),
                wait_ms: 0,
            },
        )
        .unwrap() else {
            panic!()
        };
        let progress = job.progress.unwrap();
        assert!(progress.stage.starts_with(label), "{}", progress.stage);
        assert_eq!(progress.fraction, None);
        let sessions = shared.lock().unwrap();
        assert_eq!(get(&sessions), before.0);
        assert_eq!(
            sessions.store.load_job(&child.id).unwrap().unwrap(),
            before.1
        );
    }
    {
        let mut sessions = shared.lock().unwrap();
        crate::build_record::request_cancel(&mut sessions, &parent).unwrap();
    }
    let SessionResponse::BuildVideoJob { job } = crate::build_record::execute(
        &shared,
        BuildVideoRequest::Get {
            job_id: parent.id.clone(),
            wait_ms: 0,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(job.phase, JobPhase::Cancelling);
    assert!(job.progress.unwrap().stage.contains("取消"));
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
