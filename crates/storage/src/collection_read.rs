//! Bounded metadata reads from untrusted collected folders.
use crate::PublishError;
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
pub fn read_collection_metadata(
    path: &Path,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, PublishError> {
    const MAX: u64 = 64 * 1024 * 1024;
    if cancelled() {
        return Err(PublishError::Cancelled);
    }
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.len() == 0 || before.len() > MAX {
        return Err(PublishError::Changed);
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !same(&before, &file.metadata()?) {
        return Err(PublishError::Changed);
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(PublishError::Cancelled);
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        if bytes.len() as u64 + n as u64 > before.len() {
            return Err(PublishError::Changed);
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    if bytes.len() as u64 != before.len()
        || !same(&before, &file.metadata()?)
        || !same(&before, &fs::symlink_metadata(path)?)
    {
        return Err(PublishError::Changed);
    }
    Ok(bytes)
}
