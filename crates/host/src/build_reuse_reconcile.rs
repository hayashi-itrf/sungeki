//! Explicit local reconciliation, never an engine dispatch or parent restart.
use super::{decode, reuse_target, reused_child, reused_recovery, speech_reuse};
use crate::{
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use sungeki_editor::EditReceipt;
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::StoredJob;

pub(super) fn validate(
    sessions: &Sessions,
    parent: &StoredJob,
    child: &str,
) -> Result<(), ApiError> {
    if sessions
        .store
        .load_job(&parent.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(parent)
    {
        return Err(invalid("復旧対象が更新されています"));
    }
    let (plan, state) = decode(parent)?;
    let index = state
        .receipts
        .len()
        .checked_sub(2)
        .filter(|i| *i < usize::from(plan.speech_count))
        .ok_or_else(|| invalid("復旧対象が音声工程ではありません"))?;
    if state.phase != JobPhase::Interrupted
        || state.child_id.as_deref() != Some(child)
        || plan.step_ids[index + 2] != child
        || sessions.build_runtime.owns(&parent.id)?
        || sessions
            .build_runtime
            .stopping
            .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err(invalid("中断した予約音声だけを復旧できます"));
    }
    for id in &plan.step_ids {
        if sessions.speech_runtime.owns(id)?
            || sessions.speech_resources.has_owner(id).map_err(invalid)?
            || sessions
                .render_runtime
                .flags
                .lock()
                .map_err(invalid)?
                .contains_key(id)
            || sessions
                .review_runtime
                .cancellations
                .lock()
                .map_err(invalid)?
                .contains_key(id)
        {
            return Err(invalid("制作工程がまだ実行中です"));
        }
    }
    let source = plan
        .continuation
        .as_ref()
        .ok_or_else(|| invalid("再利用の記録がありません"))?;
    if source.source_for(index as u16).is_none() {
        return Err(invalid("この工程は再利用対象ではありません"));
    }
    source.verify_current(sessions, &parent.id)
}

/// Reconcile only an existing verified original. No missing file is created and
/// no Running phase is invented. The normal editor transaction commits the
/// parent receipt, successful reused child application and project together.
pub(crate) async fn run(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<EditReceipt, ApiError> {
    let (source, old_parent, old_child, index, child, target, database) = {
        let sessions = shared.lock().map_err(invalid)?;
        let (plan, state) = decode(expected)?;
        let child = state
            .child_id
            .ok_or_else(|| invalid("復旧する予約がありません"))?;
        validate(&sessions, expected, &child)?;
        let index = (state.receipts.len() - 2) as u16;
        let source = plan
            .continuation
            .ok_or_else(|| invalid("再利用の記録がありません"))?;
        let (old_parent, old_child) = source
            .source_for(index)
            .ok_or_else(|| invalid("再利用元がありません"))?;
        let old = sessions
            .store
            .load_job(old_child)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("元音声がありません"))?;
        let (old_input, _) = crate::speech_journal::decode_input(&old)?;
        let (stored, current) = sessions.load(&expected.session_id)?;
        let target = reuse_target::prepare(
            &old_input,
            stored.revision,
            current.history().project(),
            &plan.plan.script.speech_ids[usize::from(index)],
            &expected.id,
        )?;
        super::speech_input::validate_parent_mode(
            &sessions,
            &target,
            &child,
            &expected.session_id,
            true,
        )?;
        (
            source.clone(),
            old_parent.to_owned(),
            old_child.to_owned(),
            index,
            child,
            target,
            sessions.review_database_path.clone(),
        )
    };
    let proof = speech_reuse::verify(shared, &old_parent, &old_child, &target).await?;
    let hash = target.input_hash().to_owned();
    let id = child.clone();
    let (proof, artifact, directory) = tokio::task::spawn_blocking(move || {
        let directory = crate::speech_artifacts::location(&database)?;
        let artifact = reused_recovery::inspect(
            &directory,
            &id,
            index,
            &hash,
            &source,
            &proof,
            &AtomicBool::new(false),
        )?
        .ok_or_else(|| invalid("復旧する音声原本がありません"))?;
        Ok::<_, ApiError>((proof, artifact, directory))
    })
    .await
    .map_err(invalid)??;
    {
        let mut sessions = shared.lock().map_err(invalid)?;
        validate(&sessions, expected, &child)?;
        sessions.speech_directory = Ok(directory);
    }
    reused_child::register_mode(
        shared,
        &expected.id,
        &child,
        &target,
        &proof,
        &artifact,
        true,
    )
    .await?;
    super::speech_apply::apply_mode(shared, &expected.id, &child, true).await
}
