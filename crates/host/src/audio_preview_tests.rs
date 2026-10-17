use crate::{
    audio_preview_record::{Input, State, decode},
    sessions::Sessions,
};
use std::{fs, time::Duration};
use sungeki_project::JobPhase;
use sungeki_protocol::{SessionRequest, SessionResponse};
use sungeki_storage::{JobKind, StoredJob};

#[test]
fn interrupted_audio_preview_keeps_its_work_and_never_claims_a_completed_output() {
    let root = std::env::temp_dir().join(format!("vvm-render-recovery-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let db = root.join("db");
    let mut sessions = Sessions::open(&db).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "中断復旧".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let directory = root.join("owned-work");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("partial.mp4"), b"unverified partial").unwrap();
    let input = Input {
        source_revision: 0,
        content_hash: sungeki_storage::project_hash(&snapshot.project).unwrap(),
        project: snapshot.project,
        directory: directory.clone(),
        runtime: sungeki_render::AudioRuntime {
            ffmpeg: "/unavailable/ffmpeg".into(),
            ffprobe: "/unavailable/ffprobe".into(),
            deadline: Duration::from_secs(60),
        },
    };
    let id = uuid::Uuid::new_v4().to_string();
    let mut state = State::queued();
    let mut lifecycle = state.lifecycle().unwrap();
    lifecycle.start().unwrap();
    state.update(lifecycle);
    sessions
        .store
        .create_job(&StoredJob {
            id: id.clone(),
            kind: JobKind::AudioPreview,
            session_id: snapshot.session_id,
            input: serde_json::to_vec(&input).unwrap(),
            version: 0,
            state: serde_json::to_vec(&state).unwrap(),
        })
        .unwrap();
    drop(sessions);
    let restored = Sessions::open(&db).unwrap();
    let job = restored.store.load_job(&id).unwrap().unwrap();
    let (_, state) = decode(&job).unwrap();
    assert_eq!(state.phase, JobPhase::Interrupted);
    assert!(state.output.is_none());
    assert!(state.error.unwrap().message.contains("残存確認は未実施"));
    assert_eq!(
        fs::read(directory.join("partial.mp4")).unwrap(),
        b"unverified partial"
    );
    drop(restored);
    let reopened = Sessions::open(&db).unwrap();
    assert_eq!(reopened.store.load_job(&id).unwrap().unwrap(), job);
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
