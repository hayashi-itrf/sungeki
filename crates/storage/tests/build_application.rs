use sungeki_storage::*;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("vvm-build-application-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("db"))
    }
    fn open(&self) -> RecoveryStore {
        RecoveryStore::open(&self.0).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn seed(store: &mut RecoveryStore) -> (StoredSession, StoredJob, StoredJob) {
    let session = StoredSession {
        id: "session".into(),
        revision: 1,
        version: 1,
        snapshot: b"before".to_vec().into(),
    };
    store
        .commit(
            None,
            &StoredSession {
                version: 0,
                revision: 0,
                id: session.id.clone(),
                snapshot: session.snapshot.clone(),
            },
        )
        .unwrap();
    store.commit(Some(0), &session).unwrap();
    let parent = StoredJob {
        id: "parent".into(),
        kind: JobKind::BuildVideo,
        session_id: session.id.clone(),
        input: b"recipe and fixed ids".to_vec(),
        version: 0,
        state: b"speech reserved".to_vec(),
    };
    let child = StoredJob {
        id: "speech".into(),
        kind: JobKind::Speech,
        session_id: session.id.clone(),
        input: b"captured speech bound to parent".to_vec(),
        version: 0,
        state: b"original saved, not applied".to_vec(),
    };
    store.create_job(&parent).unwrap();
    store.create_job(&child).unwrap();
    (session, parent, child)
}
fn candidate(session: &StoredSession) -> StoredSession {
    StoredSession {
        version: session.version + 1,
        revision: session.revision + 1,
        snapshot: b"audio, timing and undo receipt".to_vec().into(),
        id: session.id.clone(),
    }
}
#[test]
fn parent_child_and_project_commit_and_reopen_as_one_unit() {
    let f = Fixture::new();
    let mut store = f.open();
    let (session, parent, child) = seed(&mut store);
    let after = candidate(&session);
    assert_eq!(
        store
            .commit_build_speech_application(
                session.version,
                &after,
                &parent,
                b"next speech",
                &child,
                b"applied revision 2"
            )
            .unwrap(),
        (1, 1)
    );
    drop(store);
    let mut store = f.open();
    assert_eq!(store.load(&session.id).unwrap().unwrap(), after);
    let p = store.load_job(&parent.id).unwrap().unwrap();
    let c = store.load_job(&child.id).unwrap().unwrap();
    assert_eq!(p.state, b"next speech");
    assert_eq!(c.state, b"applied revision 2");
    assert_eq!(p.input, parent.input);
    assert_eq!(c.input, child.input);
    assert!(
        store
            .commit_build_speech_application(
                session.version,
                &after,
                &parent,
                b"duplicate",
                &child,
                b"duplicate"
            )
            .is_err()
    );
    assert_eq!(store.load(&session.id).unwrap().unwrap(), after);
    assert_eq!(store.load_job(&parent.id).unwrap().unwrap(), p);
    assert_eq!(store.load_job(&child.id).unwrap().unwrap(), c);
}
#[test]
fn competing_edits_and_either_job_update_roll_back_every_write() {
    for target in 0..3 {
        let f = Fixture::new();
        let mut store = f.open();
        let (session, parent, child) = seed(&mut store);
        let mut concurrent = f.open();
        match target {
            0 => {
                concurrent
                    .commit(Some(session.version), &candidate(&session))
                    .unwrap();
            }
            1 => {
                concurrent
                    .update_job(&parent.id, parent.kind, parent.version, b"cancelling")
                    .unwrap();
            }
            _ => {
                concurrent
                    .update_job(&child.id, child.kind, child.version, b"other application")
                    .unwrap();
            }
        }
        let before = store.load(&session.id).unwrap().unwrap();
        let p = store.load_job(&parent.id).unwrap().unwrap();
        let c = store.load_job(&child.id).unwrap().unwrap();
        assert!(
            store
                .commit_build_speech_application(
                    session.version,
                    &candidate(&session),
                    &parent,
                    b"next",
                    &child,
                    b"applied"
                )
                .is_err()
        );
        assert_eq!(store.load(&session.id).unwrap().unwrap(), before);
        assert_eq!(store.load_job(&parent.id).unwrap().unwrap(), p);
        assert_eq!(store.load_job(&child.id).unwrap().unwrap(), c);
    }
}
#[test]
fn child_write_failure_and_wrong_binding_never_leave_parent_or_snapshot_changed() {
    let f = Fixture::new();
    let mut store = f.open();
    let (session, parent, child) = seed(&mut store);
    for mode in 0..4 {
        let mut wrong = child.clone();
        match mode {
            0 => wrong.session_id = "other".into(),
            1 => wrong.kind = JobKind::Render,
            2 => wrong.input = b"different input".to_vec(),
            _ => wrong.id = parent.id.clone(),
        }
        assert!(
            store
                .commit_build_speech_application(
                    session.version,
                    &candidate(&session),
                    &parent,
                    b"next",
                    &wrong,
                    b"applied"
                )
                .is_err()
        );
    }
    let raw = rusqlite::Connection::open(&f.0).unwrap();
    raw.execute_batch("CREATE TRIGGER reject_speech BEFORE UPDATE ON jobs WHEN OLD.id='speech' BEGIN SELECT RAISE(ABORT,'simulated write failure'); END;").unwrap();
    assert!(
        store
            .commit_build_speech_application(
                session.version,
                &candidate(&session),
                &parent,
                b"next",
                &child,
                b"applied"
            )
            .is_err()
    );
    assert_eq!(store.load(&session.id).unwrap().unwrap(), session);
    assert_eq!(store.load_job(&parent.id).unwrap().unwrap(), parent);
    assert_eq!(store.load_job(&child.id).unwrap().unwrap(), child);
}
