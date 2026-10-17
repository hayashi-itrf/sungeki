use super::*;
use sungeki_project::{Checkpoint, CheckpointSummary};
use sungeki_storage::{PortableCheckpoint, StoredCheckpoint};
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub(super) fn add(f: &Fixture, name: &str) -> Checkpoint {
    let mut sessions = f.shared.lock().unwrap();
    let (_, current) = sessions.load(&f.session).unwrap();
    let project = current.history().project().clone();
    let checkpoint = Checkpoint {
        summary: CheckpointSummary {
            id: id(),
            project_id: project.id.clone(),
            name: name.into(),
            created_at: 42,
            source_revision: 0,
            content_hash: sungeki_storage::project_hash(&project).unwrap(),
        },
        project,
        asset_manifest: Vec::new(),
    };
    sessions
        .store
        .create_checkpoint(&StoredCheckpoint {
            source_session_id: f.session.clone(),
            checkpoint: checkpoint.clone(),
        })
        .unwrap();
    checkpoint
}
#[tokio::test]
async fn queued_history_is_frozen_persisted_and_replayed_after_source_deletion() {
    let f = Fixture::new();
    let first = add(&f, "開始前の保存時点");
    let runtime = f.shared.lock().unwrap().render_runtime.clone();
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    let job_id = id();
    let destination = f.root.join("package");
    let job = start(&f.shared, &f.session, &job_id, 0, &destination, false, true).unwrap();
    let (input, _) = record::decode(&job).unwrap();
    assert_eq!(input.checkpoints.len(), 1);
    assert_eq!(input.checkpoints[0].source, first);
    f.shared
        .lock()
        .unwrap()
        .store
        .delete_checkpoint(&first.summary.id, &first.project.id)
        .unwrap();
    add(&f, "開始後の保存時点");
    assert_eq!(
        start(&f.shared, &f.session, &job_id, 0, &destination, false, true)
            .unwrap()
            .input,
        job.input
    );
    assert!(
        start(
            &f.shared,
            &f.session,
            &job_id,
            0,
            &destination,
            false,
            false
        )
        .is_err()
    );
    // Read the same durable input from a fresh DB connection while work is queued.
    let reopened = sungeki_storage::RecoveryStore::open(&f.root.join("db")).unwrap();
    assert_eq!(
        reopened.load_job(&job_id).unwrap().unwrap().input,
        job.input
    );
    drop(reopened);
    drop(slot);
    let state = f.wait(&job_id).await;
    assert_eq!(state.phase, JobPhase::Succeeded);
    state.receipt.unwrap().verify(|| false).unwrap();
    assert_eq!(
        fs::read_dir(destination.join("checkpoints"))
            .unwrap()
            .count(),
        1
    );
    let path = destination.join(format!(
        "checkpoints/{}.json",
        input.checkpoints[0].collected_id
    ));
    let portable = PortableCheckpoint::from_bytes(&fs::read(path).unwrap()).unwrap();
    assert_eq!(portable.name, "開始前の保存時点");
    assert_eq!(portable.origin.checkpoint_id, first.summary.id);
    let mut tampered = f.job(&job_id);
    let mut state: record::State = serde_json::from_slice(&tampered.state).unwrap();
    state
        .receipt
        .as_mut()
        .unwrap()
        .files
        .retain(|file| !file.path.starts_with("checkpoints/"));
    tampered.state = serde_json::to_vec(&state).unwrap();
    assert!(record::decode(&tampered).is_err());
    f.shutdown().await;
}
#[tokio::test]
async fn history_capture_pages_without_truncation_and_rejects_over_limit() {
    let f = Fixture::new();
    for n in 0..127 {
        add(&f, &format!("保存時点{n}"));
    }
    let runtime = f.shared.lock().unwrap().render_runtime.clone();
    let slot = runtime.slots.clone().acquire_owned().await.unwrap();
    let job_id = id();
    let destination = f.root.join("package");
    let job = start(&f.shared, &f.session, &job_id, 0, &destination, false, true).unwrap();
    assert_eq!(record::decode(&job).unwrap().0.checkpoints.len(), 127);
    add(&f, "上限を超える保存時点");
    let rejected = id();
    let failure = start(
        &f.shared,
        &f.session,
        &rejected,
        0,
        &destination,
        false,
        true,
    )
    .unwrap_err();
    assert_eq!(
        crate::collection_execute::api::error(failure).code,
        sungeki_protocol::ErrorCode::ValidationFailed
    );
    assert!(
        f.shared
            .lock()
            .unwrap()
            .store
            .load_job(&rejected)
            .unwrap()
            .is_none()
    );
    cancel(&f.shared, &job_id).unwrap();
    drop(slot);
    assert_eq!(f.wait(&job_id).await.phase, JobPhase::Cancelled);
    assert!(!destination.exists());
    f.shutdown().await;
}
