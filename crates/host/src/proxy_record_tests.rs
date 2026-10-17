use super::*;
fn fixture() -> (StoredJob, Input) {
    let project: Project =
        serde_json::from_str(include_str!("../../../tests/fixtures/video-project.json")).unwrap();
    let input = Input {
        source_revision: 0,
        content_hash: sungeki_storage::project_hash(&project).unwrap(),
        asset_ids: vec![project.assets[0].id.clone()],
        project,
        max_edge: 640,
        encoding: sungeki_media::video_probe::frames::proxy::ProxyEncoding::Png,
        encoder_sha256: "a".repeat(64),
        ffmpeg: "/runtime/ffmpeg".into(),
        ffprobe: "/runtime/ffprobe".into(),
        work_root: "/runtime/work".into(),
        cache_root: "/runtime/cache".into(),
    };
    let job = StoredJob {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: "test-session".into(),
        kind: JobKind::Proxy,
        input: serde_json::to_vec(&input).unwrap(),
        version: 0,
        state: serde_json::to_vec(&State::queued()).unwrap(),
    };
    (job, input)
}
#[test]
fn state_requires_requested_unique_artifacts_before_success() {
    let (job, input) = fixture();
    assert!(decode(&job).is_ok());
    let mut state = State::queued();
    state.phase = JobPhase::Succeeded;
    let with = |state: &State| StoredJob {
        state: serde_json::to_vec(state).unwrap(),
        ..job.clone()
    };
    assert!(decode(&with(&state)).is_err());
    let output = Output {
        asset_id: input.asset_ids[0].clone(),
        key: input.key(&input.asset_ids[0]).unwrap(),
        sha256: "b".repeat(64),
        byte_length: 100,
    };
    state.completed.push(output.clone());
    state.completed[0].byte_length = sungeki_media::video_probe::frames::proxy::MAX_PROXY_BYTES;
    assert!(decode(&with(&state)).is_ok());
    state.completed[0].byte_length += 1;
    assert!(decode(&with(&state)).is_err());
    state.completed[0].byte_length = output.byte_length;
    assert!(decode(&with(&state)).is_ok());
    state.completed.push(output);
    assert!(decode(&with(&state)).is_err());
    state.completed.pop();
    state.completed[0].key = "0".repeat(64);
    assert!(decode(&with(&state)).is_err());
    for edit in [0, 1, 2, 3] {
        let mut changed = input.clone();
        match edit {
            0 => changed.max_edge = 0,
            1 => changed.asset_ids.push(changed.asset_ids[0].clone()),
            2 => changed.project.title = "changed".into(),
            _ => changed.ffmpeg = "relative".into(),
        }
        assert!(
            decode(&StoredJob {
                input: serde_json::to_vec(&changed).unwrap(),
                ..job.clone()
            })
            .is_err()
        );
    }
}
#[test]
fn startup_marks_nonterminal_proxy_interrupted_without_restarting_or_cleanup() {
    let root = std::env::temp_dir().join(format!("proxy-recovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("db");
    let mut sessions = Sessions::open(&db).unwrap();
    let (mut job, _) = fixture();
    let sungeki_protocol::SessionResponse::Snapshot { snapshot } = sessions
        .request(sungeki_protocol::SessionRequest::Create {
            title: "軽量動画復旧".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    job.session_id = snapshot.session_id;
    // This tests record recovery only; there is no worker launch or original decode.
    for phase in [JobPhase::Queued, JobPhase::Running, JobPhase::Cancelling] {
        let mut state = State::queued();
        state.phase = phase;
        sessions
            .store
            .create_job(&StoredJob {
                id: format!("{:?}", phase),
                state: serde_json::to_vec(&state).unwrap(),
                ..job.clone()
            })
            .unwrap();
    }
    let partial = root.join("partial.mov");
    std::fs::write(&partial, b"unverified").unwrap();
    drop(sessions);
    let sessions = Sessions::open(&db).unwrap();
    for phase in [JobPhase::Queued, JobPhase::Running, JobPhase::Cancelling] {
        let job = record(&sessions, &format!("{:?}", phase)).unwrap();
        let (_, state) = decode(&job).unwrap();
        assert_eq!(state.phase, JobPhase::Interrupted);
        assert!(state.completed.is_empty());
        assert!(state.error.is_some());
    }
    assert_eq!(std::fs::read(&partial).unwrap(), b"unverified");
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_encoding_keeps_prior_job_keys_and_rejects_unknown_formats() {
    let (job, input) = fixture();
    let id = &input.asset_ids[0];
    let old_key = input.key(id).unwrap();
    let mut document = serde_json::to_value(&input).unwrap();
    document.as_object_mut().unwrap().remove("encoding");
    let without_encoding = StoredJob {
        input: serde_json::to_vec(&document).unwrap(),
        ..job.clone()
    };
    let (restored, _) = decode(&without_encoding).unwrap();
    assert_eq!(restored.key(id).unwrap(), old_key);
    let mut new = input.clone();
    new.encoding = sungeki_media::video_probe::frames::proxy::ProxyEncoding::Qtrle;
    assert_ne!(new.key(id).unwrap(), old_key);
    let encoded = StoredJob {
        input: serde_json::to_vec(&new).unwrap(),
        ..job.clone()
    };
    assert_eq!(
        decode(&encoded).unwrap().0.key(id).unwrap(),
        new.key(id).unwrap()
    );
    document["encoding"] = serde_json::json!("unknown");
    assert!(
        decode(&StoredJob {
            input: serde_json::to_vec(&document).unwrap(),
            ..job
        })
        .is_err()
    );
}
