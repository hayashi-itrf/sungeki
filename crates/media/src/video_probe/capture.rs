//! File-backed source snapshot: bounded RAM, no decode against a mutable path.
use super::{ProbeError, ProbeRuntime};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, Metadata, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
pub(super) struct Snapshot {
    pub root: PathBuf,
    pub path: PathBuf,
    pub sha256: String,
    pub byte_length: u64,
    identity: Metadata,
}
impl Snapshot {
    pub fn new(
        source: &Path,
        runtime: &ProbeRuntime<'_>,
        check: impl Fn() -> Result<(), ProbeError>,
    ) -> Result<Self, ProbeError> {
        check()?;
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(source)?;
        let before = input.metadata()?;
        if !before.is_file() || before.len() == 0 || before.len() > runtime.max_source_bytes {
            return Err(ProbeError::Invalid("source size/type"));
        }
        let parent = runtime.root.canonicalize()?;
        crate::ensure_available_space(&parent, before.len().saturating_add(1024 * 1024))?;
        let root = parent.join(format!("video-probe-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root)?;
        let result = Self {
            path: root.join("source.media"),
            identity: fs::symlink_metadata(&root)?,
            root,
            sha256: String::new(),
            byte_length: before.len(),
        };
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&result.path)?;
        let mut hash = Sha256::new();
        let mut total = 0;
        let mut buffer = [0; 128 * 1024];
        loop {
            check()?;
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > before.len() {
                return Err(ProbeError::Changed);
            }
            out.write_all(&buffer[..n])?;
            hash.update(&buffer[..n]);
        }
        let current = fs::symlink_metadata(source)?;
        if total != before.len()
            || !current.is_file()
            || stamp(&before) != stamp(&input.metadata()?)
            || stamp(&before) != stamp(&current)
        {
            return Err(ProbeError::Changed);
        }
        out.flush()?;
        check()?;
        let mut result = result;
        result.sha256 = format!("{:x}", hash.finalize());
        Ok(result)
    }
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.root).is_ok_and(|m| {
            m.is_dir() && m.dev() == self.identity.dev() && m.ino() == self.identity.ino()
        }) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
