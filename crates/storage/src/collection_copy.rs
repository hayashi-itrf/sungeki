//! Verified original-byte copy into a caller-owned private collection stage.
use crate::{
    PublishError,
    publish_output::validate_fingerprint,
    verified_copy::{check_cancel, same_file, verified_copy},
};
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

/// Always create a new file. Reverify source bytes during copying and destination
/// bytes after sync. Failures remove only this call's own partial inode.
pub fn copy_collection_asset(
    source: &Path,
    destination: &Path,
    hash: &str,
    bytes: u64,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), PublishError> {
    validate_fingerprint(hash, bytes)?;
    check_cancel(&mut cancelled)?;
    if !source.is_absolute() || !destination.is_absolute() || destination.file_name().is_none() {
        return Err(PublishError::InvalidDestination);
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                PublishError::OutputExists
            } else {
                e.into()
            }
        })?;
    let owned = output.metadata()?;
    let result = (|| {
        verified_copy(source, hash, bytes, &mut output, &mut cancelled)?;
        output.sync_all()?;
        if !fs::symlink_metadata(destination).is_ok_and(|m| m.is_file() && same_file(&owned, &m)) {
            return Err(PublishError::Changed);
        }
        verified_copy(
            destination,
            hash,
            bytes,
            &mut std::io::sink(),
            &mut cancelled,
        )?;
        check_cancel(&mut cancelled)
    })();
    drop(output);
    if result.is_err()
        && fs::symlink_metadata(destination).is_ok_and(|m| m.is_file() && same_file(&owned, &m))
    {
        fs::remove_file(destination)?;
    }
    result
}
