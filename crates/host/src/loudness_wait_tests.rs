use super::*;
use crate::loudness_record::{Input, State};
use std::{fs, path::PathBuf, thread};
use sungeki_project::JobPhase;
use sungeki_protocol::{LoudnessJobRequest, SessionRequest};
use sungeki_storage::{JobKind, StoredJob};

fn fixture() -> (PathBuf, Arc<Mutex<Sessions>>, String) {
    let root = std::env::temp_dir().join(format!("audio-wait-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut sessions = Sessions::open(&root.join("db")).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "wait fixture".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let input = Input {
        source_revision: snapshot.revision,
        asset_id: "audio".into(),
        source_path: root.join("original.wav"),
        probe: sungeki_project::AudioProbe {
            encoding: None,
            sha256: "a".repeat(64),
            duration_us: sungeki_project::TimeUs(1_000_000),
            sample_rate: 48000,
            channels: 1,
            frames: 48000,
            byte_length: 96044,
        },
        ffmpeg: "/unused/ffmpeg".into(),
        work_root: root.clone(),
        analysis_version: sungeki_media::LOUDNESS_VERSION,
    };
    let id = uuid::Uuid::new_v4().to_string();
    sessions
        .store
        .create_job(&StoredJob {
            id: id.clone(),
            kind: JobKind::Loudness,
            session_id: snapshot.session_id,
            input: serde_json::to_vec(&input).unwrap(),
            version: 0,
            state: serde_json::to_vec(&State {
                phase: JobPhase::Queued,
                progress: None,
                output: None,
                error: None,
            })
            .unwrap(),
        })
        .unwrap();
    (root, Arc::new(Mutex::new(sessions)), id)
}

#[test]
fn loudness_wait_timeout_keeps_same_job_and_durable_state() {
    let (root, shared, id) = fixture();
    let before = shared.lock().unwrap().store.load_job(&id).unwrap();
    let started = Instant::now();
    let SessionResponse::LoudnessJob { job } = get(&shared, &id, 50).unwrap() else {
        panic!()
    };
    assert!(started.elapsed() >= Duration::from_millis(50));
    assert_eq!(job.job_id, id);
    assert_eq!(job.phase, JobPhase::Queued);
    assert_eq!(shared.lock().unwrap().store.load_job(&id).unwrap(), before);
    assert_eq!(
        get(&shared, &id, 30_001).unwrap_err().code,
        ErrorCode::ValidationFailed
    );
    assert_eq!(
        get(&shared, "missing", 30_000).unwrap_err().code,
        ErrorCode::NotFound
    );
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn loudness_wait_allows_other_requests_and_wakes_all_readers_after_cancel() {
    let (root, shared, id) = fixture();
    let readers: Vec<_> = (0..3)
        .map(|_| {
            let shared = shared.clone();
            let id = id.clone();
            thread::spawn(move || get(&shared, &id, 5_000).unwrap())
        })
        .collect();
    thread::sleep(Duration::from_millis(50));
    // A regular common-host request remains available while readers are waiting.
    Sessions::execute(
        &shared,
        SessionRequest::Create {
            title: "concurrent editing".into(),
        },
    )
    .unwrap();
    let cancelled_at = Instant::now();
    crate::loudness_api::execute(&shared, LoudnessJobRequest::Cancel { job_id: id.clone() })
        .unwrap();
    {
        let mut sessions = shared.lock().unwrap();
        let row = crate::loudness_api::record(&sessions, &id).unwrap();
        let (_, mut state) = crate::loudness_record::decode(&row).unwrap();
        let mut lifecycle = state.lifecycle().unwrap();
        lifecycle.acknowledge_cancel().unwrap();
        state.phase = lifecycle.phase();
        state.progress = lifecycle.progress().cloned();
        crate::loudness_record::save(&mut sessions, &row, &state).unwrap();
    }
    for reader in readers {
        let SessionResponse::LoudnessJob { job } = reader.join().unwrap() else {
            panic!()
        };
        assert_eq!(job.phase, JobPhase::Cancelled);
        assert_eq!(job.job_id, id);
    }
    assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    let started = Instant::now();
    get(&shared, &id, 30_000).unwrap();
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(shared);
    fs::remove_dir_all(root).unwrap();
}
