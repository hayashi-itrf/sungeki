use super::*;
use sungeki_project::Progress;
use sungeki_protocol::{BuildVideoRequest, SessionResponse};

#[tokio::test]
async fn unrelated_render_progress_is_never_shown_as_this_production() {
    for mismatch in ["session", "parent", "revision"] {
        let (root, shared, parent, _) =
            super::build_render_tests::ready_with_child(|sessions, child| {
                let (mut input, _) = crate::render_record::decode(child).unwrap();
                match mismatch {
                    "session" => {
                        let mut other = sessions.store.load(&child.session_id).unwrap().unwrap();
                        other.id = "another-session".into();
                        other.version = 0;
                        other.revision = 0;
                        other.snapshot =
                            sungeki_editor::EditSession::new(sungeki_project::Project::empty(
                                uuid::Uuid::new_v4().to_string(),
                                "Other".into(),
                            ))
                            .unwrap()
                            .snapshot()
                            .unwrap()
                            .into();
                        sessions.store.commit(None, &other).unwrap();
                        child.session_id = other.id;
                    }
                    "parent" => input.parent_job_id = Some(uuid::Uuid::new_v4().to_string()),
                    _ => input.source_revision += 1,
                }
                child.input = serde_json::to_vec(&input).unwrap();
            })
            .await;
        let error = crate::build_record::execute(
            &shared,
            BuildVideoRequest::Get {
                job_id: parent.id.clone(),
                wait_ms: 0,
            },
        )
        .unwrap_err();
        assert!(
            error.message.contains("出力子の所有または保存内容"),
            "{mismatch}: {error:?}"
        );
        drop(shared);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn render_stage_is_read_only_and_does_not_complete_or_uncancel_parent() {
    let (root, shared, parent, child) = super::build_render_tests::ready().await;
    let get = || {
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
        job
    };
    assert_eq!(get().progress.unwrap().stage, "動画出力の順番待ち");
    for (phase, stage, expected) in [
        (
            JobPhase::Running,
            "映像を作成中（32 / 900 コマ）",
            "映像を作成中（32 / 900 コマ）",
        ),
        (JobPhase::Running, "音声とBGMを調整中", "音声とBGMを調整中"),
        (
            JobPhase::Cancelled,
            "映像を作成中（32 / 900 コマ）",
            "動画出力の結果を確認中",
        ),
    ] {
        let before = {
            let mut sessions = shared.lock().unwrap();
            let row = sessions.store.load_job(&child.id).unwrap().unwrap();
            let (_, mut state) = crate::render_record::decode(&row).unwrap();
            state.phase = phase;
            state.progress = Some(Progress {
                stage: stage.into(),
                fraction: None,
            });
            crate::render_record::save(&mut sessions, &row, &state).unwrap();
            (
                sessions.store.load_job(&parent.id).unwrap(),
                sessions.store.load_job(&child.id).unwrap(),
            )
        };
        let job = get();
        assert_eq!(job.phase, JobPhase::Running);
        assert_eq!(
            job.progress,
            Some(Progress {
                stage: expected.into(),
                fraction: None
            })
        );
        let sessions = shared.lock().unwrap();
        assert_eq!(sessions.store.load_job(&parent.id).unwrap(), before.0);
        assert_eq!(sessions.store.load_job(&child.id).unwrap(), before.1);
    }
    {
        let mut sessions = shared.lock().unwrap();
        crate::build_record::request_cancel(&mut sessions, &parent).unwrap();
    }
    assert_eq!(get().phase, JobPhase::Cancelling);
    assert!(get().progress.unwrap().stage.contains("取消"));
    drop(shared);
    std::fs::remove_dir_all(root).unwrap();
}
