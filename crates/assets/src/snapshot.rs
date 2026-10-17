use crate::{
    ImageProbe, ProbeError, ProbeLimits,
    probe::{decode_image, read_image},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

#[derive(Debug)]
pub enum SnapshotError {
    Source(ProbeError),
    Destination(std::io::Error),
}
impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Destination(error) => write!(f, "検査用の原本を保存できません: {error}"),
        }
    }
}
impl std::error::Error for SnapshotError {}
impl From<ProbeError> for SnapshotError {
    fn from(error: ProbeError) -> Self {
        Self::Source(error)
    }
}
impl From<std::io::Error> for SnapshotError {
    fn from(error: std::io::Error) -> Self {
        Self::Destination(error)
    }
}

/// Capture a registered original into a new file in the caller's private job directory.
/// The caller must reserve that directory, keep it unchanged during execution, and
/// remove it on cancellation/failure or when its artifacts are no longer retained.
/// Success describes these captured bytes, not the subsequent state of the source.
/// No hard links, preview derivatives, or overwrites are used.
pub fn snapshot_image(
    source: &Path,
    destination: &Path,
    expected_hash: &str,
    limits: ProbeLimits,
) -> Result<ImageProbe, SnapshotError> {
    if !destination.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "固定素材の保存先には絶対パスが必要です",
        )
        .into());
    }
    let bytes = read_image(source, limits)?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected_hash {
        return Err(ProbeError::SourceMismatch.into());
    }
    // Validate the exact bytes being copied; never reopen the mutable source to decode.
    let (probe, _) = decode_image(&bytes, limits)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(destination)?;
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        // create_new above succeeded: never remove a pre-existing destination.
        let _ = fs::remove_file(destination);
        return Err(error.into());
    }
    Ok(probe)
}
