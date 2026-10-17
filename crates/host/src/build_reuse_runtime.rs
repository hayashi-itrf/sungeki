//! A reserved local reuse step. It never starts or contacts a speech engine.
use super::{decode, reuse_target, reused_child, reused_original, speech_reuse};
use crate::{
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_protocol::ApiError;
use sungeki_storage::StoredJob;

pub(crate) async fn run(shared: &Arc<Mutex<Sessions>>, parent: &StoredJob) -> Result<(), ApiError> {
    let result = execute(shared, parent).await;
    // The local verification/publication tasks have returned. This is evidence
    // to release an unstarted reservation on cancel, not a guessed HTTP outcome.
    if result.is_err() {
        super::speech_input::preparation::release_local_unstarted(shared, parent)?;
    }
    result
}
async fn execute(shared: &Arc<Mutex<Sessions>>, parent: &StoredJob) -> Result<(), ApiError> {
    let (source, old_parent, old_child, index, child, target, database) = {
        let sessions = shared.lock().map_err(invalid)?;
        if sessions
            .store
            .load_job(&parent.id)
            .map_err(storage_error)?
            .as_ref()
            != Some(parent)
        {
            return Err(invalid("再利用工程が更新されています"));
        }
        let (plan, state) = decode(parent)?;
        let index = state
            .receipts
            .len()
            .checked_sub(2)
            .filter(|i| *i < usize::from(plan.speech_count))
            .ok_or_else(|| invalid("再利用する合成工程ではありません"))? as u16;
        let source = plan
            .continuation
            .ok_or_else(|| invalid("続行入力がありません"))?;
        source.verify_current(&sessions, &parent.id)?;
        let (old_parent, old_child) = source
            .source_for(index)
            .ok_or_else(|| invalid("再利用元の対応がありません"))?;
        let old = sessions
            .store
            .load_job(old_child)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("再利用元がありません"))?;
        let (old_input, _) = crate::speech_journal::decode_input(&old)?;
        let (stored, current) = sessions.load(&parent.session_id)?;
        let target = reuse_target::prepare(
            &old_input,
            stored.revision,
            current.history().project(),
            &plan.plan.script.speech_ids[usize::from(index)],
            &parent.id,
        )?;
        let child = state
            .child_id
            .ok_or_else(|| invalid("再利用工程が未予約です"))?;
        target.validate_parent(&sessions, &child, &parent.session_id)?;
        let database = sessions.review_database_path.clone();
        (
            source.clone(),
            old_parent.to_owned(),
            old_child.to_owned(),
            index,
            child,
            target,
            database,
        )
    };
    let proof = speech_reuse::verify(shared, &old_parent, &old_child, &target).await?;
    let id = child.clone();
    let hash = target.input_hash().to_owned();
    let (proof, artifact, directory) =
        super::speech_input::preparation::run(shared, parent, move |flag| {
            let directory = crate::speech_artifacts::root(&database)?;
            let artifact =
                reused_original::publish(&directory, &id, index, &hash, &source, &proof, &flag)?;
            Ok((proof, artifact, directory))
        })
        .await?;
    shared.lock().map_err(invalid)?.speech_directory = Ok(directory);
    reused_child::register(shared, &parent.id, &child, &target, &proof, &artifact).await?;
    Ok(())
}
