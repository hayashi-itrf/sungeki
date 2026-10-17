use sungeki_jobs::*;
#[test]
fn normal_execution_is_terminal_and_cannot_restart() {
    for success in [true, false] {
        let mut job = Lifecycle::default();
        assert_eq!(job.finish(success), Err(TransitionError::InvalidPhase));
        job.start().unwrap();
        assert_eq!(job.start(), Err(TransitionError::InvalidPhase));
        job.finish(success).unwrap();
        assert_eq!(
            job.phase(),
            if success {
                JobPhase::Succeeded
            } else {
                JobPhase::Failed
            }
        );
        assert!(!job.request_cancel());
        assert!(job.start().is_err());
        assert!(job.finish(!success).is_err());
    }
}
#[test]
fn cancellation_wins_before_completion_but_requires_cleanup_acknowledgement() {
    for running in [false, true] {
        let mut job = Lifecycle::default();
        if running {
            job.start().unwrap();
        }
        assert!(job.request_cancel());
        assert!(job.request_cancel());
        assert_eq!(job.phase(), JobPhase::Cancelling);
        assert!(!job.phase().is_terminal());
        assert!(job.start().is_err());
        assert!(job.finish(true).is_err());
        job.acknowledge_cancel().unwrap();
        assert_eq!(job.phase(), JobPhase::Cancelled);
        assert!(!job.request_cancel());
    }
}
#[test]
fn recovery_does_not_replay_incomplete_work() {
    for phase in [JobPhase::Queued, JobPhase::Running, JobPhase::Cancelling] {
        assert_eq!(
            Lifecycle::recover(phase, None).unwrap().phase(),
            JobPhase::Interrupted
        );
    }
    for phase in [
        JobPhase::Succeeded,
        JobPhase::Failed,
        JobPhase::Cancelled,
        JobPhase::Interrupted,
    ] {
        assert_eq!(Lifecycle::recover(phase, None).unwrap().phase(), phase);
    }
}
#[test]
fn malformed_progress_does_not_replace_last_valid_progress() {
    let mut job = Lifecycle::default();
    assert!(
        job.update_progress(Progress {
            stage: "確認".into(),
            fraction: None
        })
        .is_err()
    );
    job.start().unwrap();
    job.update_progress(Progress {
        stage: "構造".into(),
        fraction: Some(0.5),
    })
    .unwrap();
    let original = job.clone();
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert_eq!(
            job.update_progress(Progress {
                stage: "構造".into(),
                fraction: Some(value)
            }),
            Err(TransitionError::InvalidProgress)
        );
        assert_eq!(job, original);
    }
    assert!(
        job.update_progress(Progress {
            stage: " ".into(),
            fraction: None
        })
        .is_err()
    );
    assert_eq!(job, original);
    job.request_cancel();
    assert!(
        job.update_progress(Progress {
            stage: "終了".into(),
            fraction: Some(1.0)
        })
        .is_err()
    );
}

#[test]
fn recovery_preserves_last_recorded_stage_and_rejects_corrupt_progress() {
    let progress = Progress {
        stage: "構造診断".into(),
        fraction: Some(0.75),
    };
    let job = Lifecycle::recover(JobPhase::Running, Some(progress.clone())).unwrap();
    assert_eq!(job.phase(), JobPhase::Interrupted);
    assert_eq!(job.progress(), Some(&progress));
    assert!(
        Lifecycle::recover(
            JobPhase::Succeeded,
            Some(Progress {
                stage: "完了".into(),
                fraction: Some(f64::NAN)
            })
        )
        .is_err()
    );
}
