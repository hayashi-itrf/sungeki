//! Read-only candidate reconciliation after local original publication.
//! This grants no permission to restart/apply an interrupted parent. The caller
//! must still recheck durable parent, target, provenance and settlement at commit.
use super::{continuation::Continuation, speech_reuse::VerifiedReuse};
use crate::{build_state::invalid, speech_state::Artifact};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_protocol::{ApiError, ErrorCode};

pub(crate) fn inspect(
    root: &Path,
    child: &str,
    index: u16,
    target_hash: &str,
    source: &Continuation,
    proof: &VerifiedReuse,
    cancelled: &AtomicBool,
) -> Result<Option<Artifact>, ApiError> {
    check_cancelled(cancelled)?;
    source.verify_proof(index, proof)?;
    if uuid::Uuid::parse_str(child)
        .ok()
        .is_none_or(|id| id.to_string() != child)
        || child == proof.child.id
        || target_hash != proof.target_input_hash
    {
        return Err(invalid("再利用原本の照合先または入力が一致しません"));
    }
    crate::speech_artifacts::check_directory(root)?;
    let root_before = stamp(root)?;
    let directory = root.join(child);
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ApiError::io(error)),
        Ok(_) => {}
    }
    crate::speech_artifacts::check_directory(&directory)?;
    let before = stamp(&directory)?;
    let mut artifact = proof.artifact.clone();
    artifact.path = directory
        .join("audio.wav")
        .to_str()
        .ok_or_else(|| invalid("保存先がUTF-8ではありません"))?
        .into();
    // The expected metadata comes from the immutable saved provenance, not from
    // trusting a WAV found under the reserved ID. Missing/partial files are errors.
    independent_file(Path::new(&artifact.path))?;
    let audio = crate::speech_original::load(&artifact)?;
    if audio.bytes() != proof.audio.bytes() {
        return Err(invalid("残っている原本が再利用元と一致しません"));
    }
    crate::speech_artifacts::check_directory(root)?;
    crate::speech_artifacts::check_directory(&directory)?;
    if root_before != stamp(root)? || before != stamp(&directory)? {
        return Err(invalid("照合中に原本の保存領域が置換されました"));
    }
    independent_file(Path::new(&artifact.path))?;
    check_cancelled(cancelled)?;
    Ok(Some(artifact))
}
fn stamp(path: &Path) -> Result<(u64, u64), ApiError> {
    let metadata = fs::symlink_metadata(path).map_err(ApiError::io)?;
    Ok((metadata.dev(), metadata.ino()))
}
fn check_cancelled(cancelled: &AtomicBool) -> Result<(), ApiError> {
    if cancelled.load(Ordering::Acquire) {
        Err(ApiError::new(
            ErrorCode::Cancelled,
            "再利用原本の照合を取り消しました",
        ))
    } else {
        Ok(())
    }
}

fn independent_file(path: &Path) -> Result<(), ApiError> {
    let metadata = fs::symlink_metadata(path).map_err(ApiError::io)?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(invalid("再利用原本が独立した通常ファイルではありません"));
    }
    Ok(())
}
