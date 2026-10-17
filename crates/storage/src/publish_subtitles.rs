//! Store a generated UTF-8 subtitle document without overwriting unrelated files.
use crate::output_stage::{Stage, commit, destination};
use crate::verified_copy::check_cancel;
use crate::{PublishError, PublishedOutput};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

/// The caller generates and validates subtitle syntax. An empty SRT is valid.
/// A repeated write succeeds only when the existing regular file matches in full.
/// Cancellation is checked before publication, never after the final file becomes visible.
pub fn publish_subtitles(
    target: &Path,
    text: &str,
    mut cancelled: impl FnMut() -> bool,
) -> Result<PublishedOutput, PublishError> {
    check_cancel(&mut cancelled)?;
    let path = destination(target, &["srt", "vtt"])?;
    let (stage, mut file) = Stage::new(path.parent().unwrap())?;
    let mut hash = Sha256::new();
    for bytes in text.as_bytes().chunks(128 * 1024) {
        check_cancel(&mut cancelled)?;
        file.write_all(bytes)?;
        hash.update(bytes);
    }
    commit(
        stage,
        file,
        path,
        &format!("{:x}", hash.finalize()),
        text.len() as u64,
        None,
        cancelled,
    )
}
