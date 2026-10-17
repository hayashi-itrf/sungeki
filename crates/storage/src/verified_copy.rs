use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use crate::publish_output::PublishError;

pub(crate) fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    same_file(a, b)
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

pub(crate) fn check_cancel(cancelled: &mut impl FnMut() -> bool) -> Result<(), PublishError> {
    if cancelled() {
        Err(PublishError::Cancelled)
    } else {
        Ok(())
    }
}

/// Bound memory and reads to the recorded output length; never follow a final symlink.
pub(crate) fn verified_copy(
    path: &Path,
    expected_hash: &str,
    expected_len: u64,
    writer: &mut impl Write,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<File, PublishError> {
    check_cancel(cancelled)?;
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(PublishError::Changed);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let initial = file.metadata()?;
    if !initial.is_file() || initial.len() != expected_len {
        return Err(PublishError::Changed);
    }
    let mut hash = Sha256::new();
    let mut length = 0u64;
    let mut buffer = [0; 128 * 1024];
    loop {
        check_cancel(cancelled)?;
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        length = length
            .checked_add(size as u64)
            .ok_or(PublishError::Changed)?;
        if length > expected_len {
            return Err(PublishError::Changed);
        }
        hash.update(&buffer[..size]);
        writer.write_all(&buffer[..size])?;
    }
    if length != expected_len
        || format!("{:x}", hash.finalize()) != expected_hash
        || !unchanged(&initial, &file.metadata()?)
        || !unchanged(&initial, &fs::symlink_metadata(path)?)
    {
        return Err(PublishError::Changed);
    }
    Ok(file)
}

pub(crate) fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}
