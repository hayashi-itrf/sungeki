use crate::{review_record, sessions::Sessions};
use sungeki_project::JobPhase;
use sungeki_protocol::*;
use sungeki_storage::{JobKind, StoredJob};

#[test]
fn terminal_review_history_does_not_block_startup_or_become_readable() {
    let root = std::env::temp_dir().join(format!("review-startup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("db");
    let mut sessions = Sessions::open(&path).unwrap();
    let SessionResponse::Snapshot { snapshot } = sessions
        .request(SessionRequest::Create {
            title: "保持する作品".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    let mut records = Vec::new();
    for phase in [
        JobPhase::Succeeded,
        JobPhase::Failed,
        JobPhase::Cancelled,
        JobPhase::Interrupted,
    ] {
        let job = StoredJob {
            id: uuid::Uuid::new_v4().to_string(),
            kind: JobKind::Review,
            session_id: snapshot.session_id.clone(),
            version: 0,
            input: b"unreadable historical input".to_vec(),
            state: serde_json::to_vec(&review_record::State {
                phase,
                progress: None,
                report: None,
                error: None,
            })
            .unwrap(),
        };
        sessions.store.create_job(&job).unwrap();
        records.push(job);
    }
    drop(sessions);
    let sessions = Sessions::open(&path).unwrap();
    assert_eq!(
        sessions
            .load(&snapshot.session_id)
            .unwrap()
            .1
            .history()
            .project(),
        &snapshot.project
    );
    for expected in records {
        let actual = sessions.store.load_job(&expected.id).unwrap().unwrap();
        assert_eq!(actual, expected);
        assert!(review_record::decode(&actual).is_err());
    }
    drop(sessions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unfinished_or_invalid_review_state_is_not_silently_skipped() {
    for state in [
        br#"{"phase":"running","progress":null,"report":null,"error":null}"#.to_vec(),
        b"invalid state".to_vec(),
    ] {
        let root =
            std::env::temp_dir().join(format!("review-startup-invalid-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("db");
        let mut sessions = Sessions::open(&path).unwrap();
        let SessionResponse::Snapshot { snapshot } = sessions
            .request(SessionRequest::Create {
                title: "検証".into(),
            })
            .unwrap()
        else {
            panic!()
        };
        let job = StoredJob {
            id: uuid::Uuid::new_v4().to_string(),
            kind: JobKind::Review,
            session_id: snapshot.session_id,
            version: 0,
            input: b"invalid".to_vec(),
            state,
        };
        sessions.store.create_job(&job).unwrap();
        drop(sessions);
        assert!(Sessions::open(&path).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
