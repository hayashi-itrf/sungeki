//! Blocking publication of a captured original. Caller must own the new parent
//! reservation and recheck it, target input and cancellation before DB commit.
use super::{continuation::Continuation, speech_reuse::VerifiedReuse};
use crate::{build_state::invalid, speech_state::Artifact};
use std::{path::Path, sync::atomic::AtomicBool};
use sungeki_protocol::ApiError;

pub(crate) fn publish(
    root: &Path,
    new_child_id: &str,
    target_index: u16,
    target_input_hash: &str,
    source: &Continuation,
    proof: &VerifiedReuse,
    cancelled: &AtomicBool,
) -> Result<Artifact, ApiError> {
    source.verify_proof(target_index, proof)?;
    if new_child_id == proof.child.id || proof.target_input_hash != target_input_hash {
        return Err(invalid("再利用先の新しい子IDまたは合成入力が一致しません"));
    }
    if let Some(artifact) = super::reused_recovery::inspect(
        root,
        new_child_id,
        target_index,
        target_input_hash,
        source,
        proof,
        cancelled,
    )? {
        return Ok(artifact); // Exact saved publication; no rewrite or application.
    }
    let lip_sync = proof
        .artifact
        .lip_sync
        .clone()
        .ok_or_else(|| invalid("再利用元の口パク情報がありません"))?;
    crate::speech_artifacts::publish(root, new_child_id, &proof.audio, lip_sync, cancelled)
}
