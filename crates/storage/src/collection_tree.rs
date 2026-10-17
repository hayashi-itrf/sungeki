//! Flush the private collection tree before its atomic directory publication.
use crate::{
    PublishError,
    verified_copy::{check_cancel, same_file},
};
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
pub(crate) fn sync(root: &Path, cancelled: &mut impl FnMut() -> bool) -> Result<(), PublishError> {
    visit(root, 0, &mut 100_000, cancelled)
}
fn visit(
    path: &Path,
    depth: usize,
    remaining: &mut usize,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), PublishError> {
    check_cancel(cancelled)?;
    if depth > 16 || *remaining == 0 {
        return Err(PublishError::InvalidRange);
    }
    *remaining -= 1;
    let meta = fs::symlink_metadata(path)?;
    if (meta.is_file() && meta.nlink() != 1) || (!meta.is_file() && !meta.is_dir()) {
        return Err(PublishError::Changed);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !same_file(&meta, &file.metadata()?) {
        return Err(PublishError::Changed);
    }
    if meta.is_dir() {
        for item in fs::read_dir(path)? {
            visit(&item?.path(), depth + 1, remaining, cancelled)?;
        }
    }
    check_cancel(cancelled)?;
    if !same_file(&meta, &fs::symlink_metadata(path)?) {
        return Err(PublishError::Changed);
    }
    file.sync_all()?;
    Ok(())
}
