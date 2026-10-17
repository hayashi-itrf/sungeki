use super::*;
use std::fs;
use sungeki_media::LoudnessUnavailable;
use sungeki_project::TimeUs;
use sungeki_protocol::{SessionRequest, SessionResponse};
fn input(root: &std::path::Path) -> Input {
    Input {
        source_revision: 0,
        asset_id: "audio".into(),
        source_path: root.join("source.wav"),
        probe: AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: TimeUs(1_000_000),
            sample_rate: 48000,
            channels: 1,
            frames: 48000,
            byte_length: 192044,
        },
        ffmpeg: "/unused/ffmpeg".into(),
        work_root: root.to_path_buf(),
        analysis_version: LOUDNESS_VERSION,
    }
}
fn output(input: &Input) -> LoudnessAnalysis {
    LoudnessAnalysis {
        version: LOUDNESS_VERSION,
        method: "ffmpeg_ebur128_summary_192k".into(),
        source_sha256: input.probe.sha256.clone(),
        source_frames: input.probe.frames,
        sample_rate: input.probe.sample_rate,
        channels: input.probe.channels,
        source_pre_gain: true,
        dual_mono: false,
        measurement: LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::BelowGateOrSilence,
            true_peak_db: None,
        },
    }
}
fn job(input: &Input, phase: JobPhase) -> StoredJob {
    StoredJob {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: "test".into(),
        kind: JobKind::Loudness,
        input: serde_json::to_vec(input).unwrap(),
        version: 0,
        state: serde_json::to_vec(&State {
            phase,
            progress: None,
            error: None,
            output: (phase == JobPhase::Succeeded).then(|| output(input)),
        })
        .unwrap(),
    }
}
#[test]
fn decode_rejects_wrong_source_version_units_and_impossible_measurements() {
    let input = input(std::path::Path::new("/unused"));
    let original = job(&input, JobPhase::Succeeded);
    assert!(decode(&original).is_ok());
    for (field, value) in [
        ("source_sha256", serde_json::json!("b".repeat(64))),
        ("source_frames", serde_json::json!(47999)),
        ("sample_rate", serde_json::json!(44100)),
        ("channels", serde_json::json!(2)),
        ("version", serde_json::json!(1)),
        ("method", serde_json::json!("unknown")),
        ("source_pre_gain", serde_json::json!(false)),
        ("dual_mono", serde_json::json!(true)),
        (
            "measurement",
            serde_json::json!({"status":"not_measurable","reason":"too_short","true_peak_db":null}),
        ),
        (
            "measurement",
            serde_json::json!({"status":"measured","integrated_lufs":-70.,"true_peak_db":-30.,"range_lu":0.,"threshold_lufs":-80.}),
        ),
    ] {
        let mut state: serde_json::Value = serde_json::from_slice(&original.state).unwrap();
        state["output"][field] = value;
        assert!(
            decode(&StoredJob {
                state: serde_json::to_vec(&state).unwrap(),
                ..original.clone()
            })
            .is_err(),
            "{field}"
        );
    }
    assert!(
        decode(&StoredJob {
            kind: JobKind::Render,
            ..original
        })
        .is_err()
    );
}
#[test]
fn restart_marks_incomplete_jobs_interrupted_and_preserves_completed_unmeasurable_result() {
    let root = std::env::temp_dir().join(format!("vvm-loudness-recovery-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "音量解析復旧".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let input = input(&root);
    fs::write(&input.source_path, b"untouched fixture, no engine").unwrap();
    let mut jobs = vec![];
    for phase in [
        JobPhase::Queued,
        JobPhase::Running,
        JobPhase::Cancelling,
        JobPhase::Succeeded,
        JobPhase::Cancelled,
    ] {
        let mut job = job(&input, phase);
        job.session_id = snapshot.session_id.clone();
        sessions.store.create_job(&job).unwrap();
        jobs.push(job);
    }
    drop(sessions);
    let restored = Sessions::open(&root.join("db")).unwrap();
    let mut recovered = vec![];
    for original in &jobs {
        let row = restored.store.load_job(&original.id).unwrap().unwrap();
        let (_, before) = decode(original).unwrap();
        let (_, after) = decode(&row).unwrap();
        if before.phase.is_terminal() {
            assert_eq!(row, *original);
        } else {
            assert_eq!(after.phase, JobPhase::Interrupted);
            assert!(after.output.is_none());
            assert!(after.error.unwrap().message.contains("残存確認は未実施"));
        }
        recovered.push(row);
    }
    assert_eq!(
        fs::read(&input.source_path).unwrap(),
        b"untouched fixture, no engine"
    );
    drop(restored);
    let reopened = Sessions::open(&root.join("db")).unwrap();
    for row in recovered {
        assert_eq!(reopened.store.load_job(&row.id).unwrap().unwrap(), row);
    }
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
