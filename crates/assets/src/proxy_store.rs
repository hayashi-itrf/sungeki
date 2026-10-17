//! Private cache publication; no Project edits, original writes or fallback render.
#[path = "proxy_manifest.rs"]
mod manifest;
use crate::proxy::ProxyKey;
pub use manifest::ProxyManifest;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
const MAX_MANIFEST: u64 = 16 * 1024 * 1024;
fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}
fn check(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "proxy publication cancelled",
        ))
    } else {
        Ok(())
    }
}
struct OwnedDirectory {
    path: PathBuf,
    device: u64,
    inode: u64,
    committed: bool,
}
impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        if !self.committed
            && fs::symlink_metadata(&self.path)
                .is_ok_and(|m| m.is_dir() && m.dev() == self.device && m.ino() == self.inode)
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
fn open(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("cache entry is not a regular file"));
    }
    Ok(file)
}
fn artifact(
    path: &Path,
    manifest: &ProxyManifest,
    mut output: Option<&mut File>,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let mut input = open(path)?;
    if input.metadata()?.len() != manifest.derived.byte_length {
        return Err(invalid("proxy size changed"));
    }
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        check(cancel)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > manifest.derived.byte_length {
            return Err(invalid("proxy grew during read"));
        }
        hash.update(&buffer[..count]);
        if let Some(file) = output.as_deref_mut() {
            file.write_all(&buffer[..count])?;
        }
    }
    if total != manifest.derived.byte_length
        || format!("{:x}", hash.finalize()) != manifest.derived.sha256
    {
        return Err(invalid("proxy content changed"));
    }
    check(cancel)
}

/// Publish into a new key directory. Existing entries, including incomplete crash
/// remnants, are never replaced. The caller may retry after explicit cache cleanup.
pub fn publish(
    root: &Path,
    manifest: &ProxyManifest,
    source: &Path,
    cancel: &AtomicBool,
) -> io::Result<PathBuf> {
    check(cancel)?;
    if !root.is_absolute() || !source.is_absolute() {
        return Err(invalid("absolute cache paths required"));
    }
    let key = manifest.key().map_err(invalid)?;
    let bytes = serde_json::to_vec(manifest).map_err(invalid)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err(invalid("proxy manifest too large"));
    }
    let root = root.canonicalize()?;
    let path = root.join(key.as_str());
    fs::DirBuilder::new().mode(0o700).create(&path)?;
    let metadata = fs::symlink_metadata(&path)?;
    let mut owned = OwnedDirectory {
        path: path.clone(),
        device: metadata.dev(),
        inode: metadata.ino(),
        committed: false,
    };
    let mut video = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.join("video.mov"))?;
    artifact(source, manifest, Some(&mut video), cancel)?;
    video.sync_all()?;
    let mut record = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.join("manifest.pending"))?;
    record.write_all(&bytes)?;
    record.sync_all()?;
    check(cancel)?;
    fs::rename(path.join("manifest.pending"), path.join("manifest.json"))?;
    File::open(&path)?.sync_all()?;
    File::open(&root)?.sync_all()?;
    owned.committed = true;
    Ok(path)
}

/// Hash-verified candidate only. Callers must reopen/reprobe the same artifact and
/// verify the saved source/proxy frame map before treating it as a usable proxy.
pub fn read_candidate(
    root: &Path,
    key: &ProxyKey,
    cancel: &AtomicBool,
) -> io::Result<ProxyManifest> {
    check(cancel)?;
    if !root.is_absolute() {
        return Err(invalid("absolute cache root required"));
    }
    let path = root.canonicalize()?.join(key.as_str());
    if !fs::symlink_metadata(&path)?.is_dir() {
        return Err(invalid("proxy directory replaced"));
    }
    let mut input = open(&path.join("manifest.json"))?.take(MAX_MANIFEST + 1);
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err(invalid("proxy manifest too large"));
    }
    let manifest: ProxyManifest = serde_json::from_slice(&bytes).map_err(invalid)?;
    if manifest.key().map_err(invalid)? != *key {
        return Err(invalid("proxy key mismatch"));
    }
    artifact(&path.join("video.mov"), &manifest, None, cancel)?;
    Ok(manifest)
}

#[cfg(test)]
#[path = "proxy_store_tests.rs"]
mod tests;
