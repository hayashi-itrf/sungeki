use super::*;
use crate::collection_execute::execute_recorded;
use std::{cell::RefCell, fs, path::PathBuf};
use sungeki_assets::CollectionLimits;
use sungeki_storage::{PublishError, StoredSession};
struct Fixture {
    root: PathBuf,
    input: Input,
    id: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("collection-job-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let project = Project::empty(uuid::Uuid::new_v4().to_string(), "収集復旧試験".into());
        let input = Input {
            source_revision: 0,
            dry_run: false,
            include_checkpoints: false,
            checkpoints: Vec::new(),
            content_hash: sungeki_storage::project_hash(&project).unwrap(),
            project,
            destination: root.join("output"),
            collected_project_id: uuid::Uuid::new_v4().to_string(),
        };
        Self {
            root,
            input,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }
    fn store(&self) -> RecoveryStore {
        RecoveryStore::open(&self.root.join("recovery.sqlite")).unwrap()
    }
    fn seed(&self, store: &mut RecoveryStore) {
        store
            .commit(
                None,
                &StoredSession {
                    id: "session".into(),
                    revision: 0,
                    version: 0,
                    snapshot: b"fixture".to_vec().into(),
                },
            )
            .unwrap();
        create(store, self.id.clone(), "session".into(), &self.input).unwrap();
        let job = self.job(store);
        let (_, mut state) = decode(&job).unwrap();
        state.start().unwrap();
        save(store, &job, &state).unwrap();
    }
    fn job(&self, store: &RecoveryStore) -> StoredJob {
        store.load_job(&self.id).unwrap().unwrap()
    }
    fn publish(&self, store: &RefCell<RecoveryStore>, abort: bool) -> Result<Receipt, Error> {
        execute_recorded(
            &self.input.project,
            &self.input.destination,
            CollectionLimits::default(),
            &self.input.collected_project_id,
            || false,
            |receipt| {
                let mut store = store.borrow_mut();
                let job = self.job(&store);
                let (_, mut state) = decode(&job)?;
                state.intent(receipt)?;
                save(&mut store, &job, &state)?;
                if abort {
                    Err(PublishError::Cancelled.into())
                } else {
                    Ok(())
                }
            },
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn committed_folder_is_recovered_from_persisted_intent_and_replay_keeps_result() {
    let f = Fixture::new();
    let mut store = f.store();
    f.seed(&mut store);
    let store = RefCell::new(store);
    f.publish(&store, false).unwrap();
    // Deliberately omit recording success, then reopen the durable database.
    drop(store);
    let mut store = f.store();
    let job = f.job(&store);
    assert_eq!(decode(&job).unwrap().1.phase, JobPhase::Running);
    let recovered = recover(&job, || false).unwrap();
    assert_eq!(recovered.phase, JobPhase::Succeeded);
    save(&mut store, &job, &recovered).unwrap();
    let replay = create(&mut store, f.id.clone(), "session".into(), &f.input).unwrap();
    assert_eq!(decode(&replay).unwrap().1.phase, JobPhase::Succeeded);
    let mut different = f.input.clone();
    different.destination = f.root.join("different");
    assert!(create(&mut store, f.id.clone(), "session".into(), &different).is_err());
    assert_eq!(
        store
            .list_jobs(Some(JobKind::Collection), None, None, 10)
            .unwrap(),
        vec![f.id.clone()]
    );
}
#[test]
fn unpublished_or_modified_results_are_interrupted_without_deletion() {
    for publish in [false, true] {
        let f = Fixture::new();
        let mut store = f.store();
        f.seed(&mut store);
        let store = RefCell::new(store);
        let result = f.publish(&store, !publish);
        if publish {
            result.unwrap();
            fs::write(f.input.destination.join("manifest.json"), b"changed").unwrap();
        } else {
            assert!(result.is_err());
        }
        drop(store);
        let mut store = f.store();
        let job = f.job(&store);
        let state = recover(&job, || false).unwrap();
        assert_eq!(state.phase, JobPhase::Interrupted);
        save(&mut store, &job, &state).unwrap();
        if publish {
            assert_eq!(
                fs::read(f.input.destination.join("manifest.json")).unwrap(),
                b"changed"
            );
        } else {
            assert!(!f.input.destination.exists());
        }
    }
}
#[test]
fn late_cancel_preserves_publication_and_old_versions_cannot_overwrite_it() {
    let f = Fixture::new();
    let mut store = f.store();
    f.seed(&mut store);
    let store = RefCell::new(store);
    let receipt = f.publish(&store, false).unwrap();
    let mut store = store.into_inner();
    let old = f.job(&store);
    let (_, mut cancelling) = decode(&old).unwrap();
    cancelling.request_cancel();
    save(&mut store, &old, &cancelling).unwrap();
    let current = f.job(&store);
    let (_, mut state) = decode(&current).unwrap();
    state.published(&receipt).unwrap();
    save(&mut store, &current, &state).unwrap();
    assert!(save(&mut store, &old, &cancelling).is_err());
    assert_eq!(decode(&f.job(&store)).unwrap().1.phase, JobPhase::Succeeded);
    receipt.verify(|| false).unwrap();
}
#[test]
fn wrong_receipt_is_rejected_and_cancelled_scan_does_not_change_database() {
    let f = Fixture::new();
    let mut store = f.store();
    f.seed(&mut store);
    let store = RefCell::new(store);
    let receipt = f.publish(&store, false).unwrap();
    let mut store = store.into_inner();
    let job = f.job(&store);
    let (_, mut state) = decode(&job).unwrap();
    let mut wrong = receipt;
    wrong.destination = f.root.join("wrong");
    state.receipt = Some(wrong);
    assert!(save(&mut store, &job, &state).is_err());
    assert!(recover(&job, || true).is_err());
    assert_eq!(f.job(&store), job);
}

#[test]
fn restart_reports_recorded_stage_without_deleting_replaced_or_owned_content() {
    for replaced in [false, true] {
        let f = Fixture::new();
        let mut store = f.store();
        f.seed(&mut store);
        let stage = sungeki_storage::CollectionStage::new(&f.input.destination).unwrap();
        let marker = stage.path().join("partial");
        fs::write(&marker, b"unfinished").unwrap();
        let job = f.job(&store);
        let (_, mut state) = decode(&job).unwrap();
        state.stage_created(&stage.record().unwrap()).unwrap();
        save(&mut store, &job, &state).unwrap();
        drop(store);
        if replaced {
            fs::rename(stage.path(), f.root.join("kept-stage")).unwrap();
            fs::create_dir(stage.path()).unwrap();
            fs::write(&marker, b"replacement").unwrap();
        }
        let mut store = f.store();
        let job = f.job(&store);
        let state = recover(&job, || false).unwrap();
        assert_eq!(state.phase, JobPhase::Interrupted);
        assert_eq!(
            state.error.as_ref().unwrap().details["staging_status"],
            if replaced { "changed" } else { "owned" }
        );
        assert_eq!(
            state.error.as_ref().unwrap().details["staging_path"],
            stage.path().to_string_lossy()
        );
        assert!(marker.exists());
        save(&mut store, &job, &state).unwrap();
        assert!(decode(&f.job(&store)).unwrap().1.stage.is_some());
        drop(stage); // Live guard cleans its own directory only; replacement remains.
        if replaced {
            assert_eq!(fs::read(marker).unwrap(), b"replacement");
        }
    }
}
