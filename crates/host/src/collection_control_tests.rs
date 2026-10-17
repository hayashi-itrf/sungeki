use super::*;
use std::{fs, path::PathBuf, time::Duration};
use sungeki_project::JobPhase;
use sungeki_protocol::{SessionRequest, SessionResponse};
struct Fixture {
    root: PathBuf,
    shared: Arc<Mutex<Sessions>>,
    session: String,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("collection-control-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut sessions = Sessions::open(&root.join("db")).unwrap();
        let SessionResponse::Snapshot { snapshot } = sessions
            .request(SessionRequest::Create {
                title: "収集worker検証".into(),
            })
            .unwrap()
        else {
            panic!()
        };
        Self {
            root,
            shared: Arc::new(Mutex::new(sessions)),
            session: snapshot.session_id,
        }
    }
    fn job(&self, id: &str) -> StoredJob {
        load(&self.shared.lock().unwrap(), id).unwrap()
    }
    async fn wait(&self, id: &str) -> record::State {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let (_, state) = record::decode(&self.job(id)).unwrap();
                if state.phase.is_terminal() {
                    return state;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn shutdown(&self) {
        let runtime = self.shared.lock().unwrap().render_runtime.clone();
        runtime.shutdown().await;
        assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
        assert!(runtime.flags.lock().unwrap().is_empty());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[tokio::test]
async fn worker_publishes_frozen_snapshot_and_replays_without_duplicate_output() {
    let f = Fixture::new();
    let id = uuid::Uuid::new_v4().to_string();
    // Hold the worker slot to prove start returns before copying and editing works.
    let runtime = f.shared.lock().unwrap().render_runtime.clone();
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    let target = f.root.join("package");
    let job = start(&f.shared, &f.session, &id, 0, &target, false, false).unwrap();
    assert_eq!(record::decode(&job).unwrap().1.phase, JobPhase::Queued);
    assert!(!target.exists());
    f.shared
        .lock()
        .unwrap()
        .request(SessionRequest::Edit {
            session_id: f.session.clone(),
            edit: sungeki_protocol::EditRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_revision: 0,
                action: sungeki_protocol::EditAction::Apply {
                    commands: vec![sungeki_editor::Command::RenameProject {
                        title: "編集中の新しい名前".into(),
                    }],
                },
            },
        })
        .unwrap();
    assert_eq!(
        start(&f.shared, &f.session, &id, 0, &target, false, false).unwrap(),
        job
    );
    assert!(start(&f.shared, &f.session, &id, 1, &target, false, false).is_err());
    drop(slot);
    let state = f.wait(&id).await;
    assert_eq!(state.phase, JobPhase::Succeeded);
    assert_eq!(
        state.stage.as_ref().unwrap().inspect().unwrap(),
        sungeki_storage::CollectionStagePresence::Missing
    );
    state.receipt.unwrap().verify(|| false).unwrap();
    let loaded = sungeki_storage::load_document(&target.join("project.vvm.json")).unwrap();
    assert_eq!(loaded.project.title, "収集worker検証");
    let replay = start(&f.shared, &f.session, &id, 0, &target, false, false).unwrap();
    assert_eq!(
        record::decode(&replay).unwrap().1.phase,
        JobPhase::Succeeded
    );
    assert_eq!(cancel(&f.shared, &id).unwrap(), replay);
    f.shutdown().await;
}
#[tokio::test]
async fn queued_cancel_and_shutdown_never_publish_and_release_registration() {
    for (shutdown, dry_run) in [(false, false), (true, false), (false, true), (true, true)] {
        let f = Fixture::new();
        let id = uuid::Uuid::new_v4().to_string();
        let runtime = f.shared.lock().unwrap().render_runtime.clone();
        let slot = runtime.slots.clone().acquire_owned().await.unwrap();
        let target = f.root.join("package");
        start(&f.shared, &f.session, &id, 0, &target, dry_run, false).unwrap();
        if shutdown {
            // Signal the same stop/flag state used by Runtime::shutdown before releasing slot.
            runtime.stopping.store(true, Ordering::SeqCst);
            runtime.flags.lock().unwrap()[&id].store(true, Ordering::SeqCst);
        } else {
            cancel(&f.shared, &id).unwrap();
        }
        drop(slot);
        assert_eq!(f.wait(&id).await.phase, JobPhase::Cancelled);
        assert!(!target.exists());
        f.shutdown().await;
        assert!(
            start(
                &f.shared,
                &f.session,
                &uuid::Uuid::new_v4().to_string(),
                0,
                &target,
                false,
                false
            )
            .is_err()
        );
    }
}
#[tokio::test]
async fn existing_destination_fails_without_changing_user_files() {
    let f = Fixture::new();
    let target = f.root.join("package");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), b"keep").unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    start(&f.shared, &f.session, &id, 0, &target, false, false).unwrap();
    assert_eq!(f.wait(&id).await.phase, JobPhase::Failed);
    assert_eq!(fs::read(target.join("keep")).unwrap(), b"keep");
    f.shutdown().await;
}

#[tokio::test]
async fn startup_reconciles_publication_saved_without_final_success_state() {
    let f = Fixture::new();
    let checkpoint = frozen_tests::add(&f, "公開後の復旧試験");
    let id = uuid::Uuid::new_v4().to_string();
    let input = {
        let mut sessions = f.shared.lock().unwrap();
        let (_, current) = sessions.load(&f.session).unwrap();
        let project = current.history().project().clone();
        let input = record::Input {
            source_revision: 0,
            dry_run: false,
            include_checkpoints: true,
            checkpoints: vec![crate::collection_execute::history::FrozenCheckpoint {
                collected_id: uuid::Uuid::new_v4().to_string(),
                source: checkpoint,
            }],
            content_hash: sungeki_storage::project_hash(&project).unwrap(),
            project,
            destination: f.root.join("recovered"),
            collected_project_id: uuid::Uuid::new_v4().to_string(),
        };
        let job =
            record::create(&mut sessions.store, id.clone(), f.session.clone(), &input).unwrap();
        let (_, mut state) = record::decode(&job).unwrap();
        state.start().unwrap();
        record::save(&mut sessions.store, &job, &state).unwrap();
        input
    };
    crate::collection_execute::execute_with_history(
        &input.project,
        &input.checkpoints,
        &input.destination,
        sungeki_assets::CollectionLimits::default(),
        &input.collected_project_id,
        || false,
        |receipt| {
            let mut sessions = f.shared.lock().unwrap();
            let job = load(&sessions, &id)?;
            let (_, mut state) = record::decode(&job)?;
            state.intent(receipt)?;
            record::save(&mut sessions.store, &job, &state)
        },
    )
    .unwrap();
    // Reopen the durable store in a fresh Sessions owner; the original owner is idle.
    let reopened = Arc::new(Mutex::new(Sessions::open(&f.root.join("db")).unwrap()));
    crate::collection_execute::recovery::launch(reopened.clone()).unwrap();
    let runtime = reopened.lock().unwrap().render_runtime.clone();
    tokio::time::timeout(Duration::from_secs(10), async {
        while runtime.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let job = load(&reopened.lock().unwrap(), &id).unwrap();
    let (_, state) = record::decode(&job).unwrap();
    assert_eq!(state.phase, JobPhase::Succeeded);
    state.receipt.unwrap().verify(|| false).unwrap();
    runtime.shutdown().await;
    f.shutdown().await;
}

#[path = "collection_frozen_tests.rs"]
mod frozen_tests;
