//! Register a locally reused result without claiming an external synthesis ran.
//! Parent/child/project application remains the existing atomic build apply step.
use super::{decode, speech_reuse::VerifiedReuse};
use crate::{
    build_state::invalid,
    sessions::{Sessions, storage_error},
    speech_journal::Input,
    speech_state::{Application, Artifact, Execution, State},
};
use std::sync::{Arc, Mutex};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) async fn register(
    shared: &Arc<Mutex<Sessions>>,
    parent_id: &str,
    child_id: &str,
    target: &Input,
    proof: &VerifiedReuse,
    artifact: &Artifact,
) -> Result<StoredJob, ApiError> {
    register_mode(shared, parent_id, child_id, target, proof, artifact, false).await
}

pub(super) async fn register_mode(
    shared: &Arc<Mutex<Sessions>>,
    parent_id: &str,
    child_id: &str,
    target: &Input,
    proof: &VerifiedReuse,
    artifact: &Artifact,
    recover_reused: bool,
) -> Result<StoredJob, ApiError> {
    let encoded = serde_json::to_vec(target).map_err(invalid)?;
    let parent = {
        let sessions = shared.lock().map_err(invalid)?;
        let parent = sessions
            .store
            .load_job(parent_id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("続行先の親がありません"))?;
        if target.parent_job_id() != Some(parent_id)
            || target.input_hash() != proof.target_input_hash
        {
            return Err(invalid("再利用先の合成入力が一致しません"));
        }
        let (plan, _) = decode(&parent)?;
        let index = plan
            .step_ids
            .iter()
            .position(|id| id == child_id)
            .filter(|i| *i >= 2 && *i < usize::from(plan.speech_count) + 2)
            .ok_or_else(|| invalid("再利用先の合成工程ではありません"))?;
        let source = plan
            .continuation
            .as_ref()
            .ok_or_else(|| invalid("続行入力がありません"))?;
        source.verify_proof((index - 2) as u16, proof)?;
        let path = sessions
            .speech_directory
            .as_ref()
            .map_err(Clone::clone)?
            .join(child_id)
            .join("audio.wav");
        let mut expected = proof.artifact.clone();
        expected.path = path
            .to_str()
            .ok_or_else(|| invalid("保存先がUTF-8ではありません"))?
            .into();
        if serde_json::to_value(artifact).map_err(invalid)?
            != serde_json::to_value(expected).map_err(invalid)?
        {
            return Err(invalid("引き継いだ原本が再利用証拠と一致しません"));
        }
        if let Some(existing) = sessions.store.load_job(child_id).map_err(storage_error)? {
            let (_, state) = crate::speech_journal::decode_input(&existing)?;
            if existing.session_id != parent.session_id
                || existing.input != encoded
                || state.execution != Execution::Reused
                || state.phase != JobPhase::Succeeded
                || serde_json::to_value(&state.artifact).map_err(invalid)?
                    != serde_json::to_value(Some(artifact)).map_err(invalid)?
            {
                return Err(invalid("同じ子IDが別の処理に使われています"));
            }
            return Ok(existing); // Receipt replay does not reapply, rewrite, or dispatch.
        }
        source.verify_current(&sessions, parent_id)?;
        super::speech_input::validate_parent_mode(
            &sessions,
            target,
            child_id,
            &parent.session_id,
            recover_reused,
        )?;
        parent
    };
    let original = artifact.clone();
    tokio::task::spawn_blocking(move || crate::speech_original::load(&original))
        .await
        .map_err(invalid)??;
    let mut sessions = shared.lock().map_err(invalid)?;
    if sessions
        .store
        .load_job(parent_id)
        .map_err(storage_error)?
        .as_ref()
        != Some(&parent)
    {
        return Err(invalid("原本確認中に続行先の親が更新されました"));
    }
    let (plan, _) = decode(&parent)?;
    plan.continuation
        .as_ref()
        .ok_or_else(|| invalid("続行入力がありません"))?
        .verify_current(&sessions, parent_id)?;
    super::speech_input::validate_parent_mode(
        &sessions,
        target,
        child_id,
        &parent.session_id,
        recover_reused,
    )?;
    let state = State {
        phase: JobPhase::Succeeded,
        progress: None,
        execution: Execution::Reused,
        artifact: Some(artifact.clone()),
        application: Application::PendingConflict,
        error: None,
    };
    let row = StoredJob {
        id: child_id.into(),
        session_id: parent.session_id,
        kind: JobKind::Speech,
        version: 0,
        input: encoded,
        state: crate::speech_journal::encode(&state)?,
    };
    crate::speech_journal::decode_input(&row)?;
    // A new reused result starts terminal; ordinary queued jobs cannot transition
    // to Reused through speech_transition or bypass external settlement.
    if sessions
        .store
        .load_job(child_id)
        .map_err(storage_error)?
        .is_some()
    {
        return Err(invalid(
            "原本確認中に子IDが使用されました。記録を再確認してください",
        ));
    }
    sessions.store.create_job(&row).map_err(storage_error)
}
