//! Publication copies a host-verified output; it does not validate media codecs or edit Project.
use std::path::{Path, PathBuf};

use crate::output_stage::{Stage, commit, destination};
use crate::verified_copy::{check_cancel, verified_copy};

#[derive(Debug)]
pub enum PublishError {
    InvalidDestination,
    InvalidRange,
    InvalidFingerprint,
    Changed,
    OutputExists,
    Cancelled,
    Io(std::io::Error),
    /// The complete file is visible, but directory durability could not be confirmed.
    DurabilityUncertain {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl From<std::io::Error> for PublishError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PublishedOutput {
    pub path: PathBuf,
    pub already_present: bool,
}

/// Copy a completed output into a same-directory stage, then atomically create the destination.
/// Existing differing files are never overwritten. Repeating after a lost response is safe:
/// an existing regular file must match the recorded hash and size in full.
/// `cancelled` may also check a deadline; cancellation is observed only before publication.
pub fn publish_output(
    source: &Path,
    target: &Path,
    sha256: &str,
    byte_length: u64,
    cancelled: impl FnMut() -> bool,
) -> Result<PublishedOutput, PublishError> {
    publish_media(source, target, sha256, byte_length, &["mp4"], cancelled)
}

/// Publish a host-verified PNG with the same non-overwriting guarantees as video.
pub fn publish_png_output(
    source: &Path,
    target: &Path,
    sha256: &str,
    byte_length: u64,
    cancelled: impl FnMut() -> bool,
) -> Result<PublishedOutput, PublishError> {
    publish_media(source, target, sha256, byte_length, &["png"], cancelled)
}

fn publish_media(
    source: &Path,
    target: &Path,
    sha256: &str,
    byte_length: u64,
    extensions: &[&str],
    mut cancelled: impl FnMut() -> bool,
) -> Result<PublishedOutput, PublishError> {
    validate_fingerprint(sha256, byte_length)?;
    check_cancel(&mut cancelled)?;
    let path = destination(target, extensions)?;
    let parent = path.parent().unwrap();
    let (stage, mut file) = Stage::new(parent)?;
    let original = verified_copy(source, sha256, byte_length, &mut file, &mut cancelled)?;
    commit(
        stage,
        file,
        path,
        sha256,
        byte_length,
        Some(&original),
        cancelled,
    )
}

pub(crate) fn validate_fingerprint(sha256: &str, byte_length: u64) -> Result<(), PublishError> {
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || byte_length == 0
    {
        return Err(PublishError::InvalidFingerprint);
    }
    Ok(())
}

/// Recheck an existing completed output without copying or changing it.
/// This verifies bytes and file stability, not media codecs or visual quality.
pub fn verify_output(
    source: &Path,
    sha256: &str,
    byte_length: u64,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), PublishError> {
    validate_fingerprint(sha256, byte_length)?;
    verified_copy(
        source,
        sha256,
        byte_length,
        &mut std::io::sink(),
        &mut cancelled,
    )?;
    Ok(())
}
