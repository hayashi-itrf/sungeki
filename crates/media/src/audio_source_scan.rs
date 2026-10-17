//! Bounded, stable source scan shared by capture and freshness observation.
use crate::WaveformCacheError;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use sungeki_project::AudioProbe;

/// Verifies stable original bytes only; does not decode audio, images, or PSD layers.
pub fn source_identity(
    path: &Path,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<(String, u64), WaveformCacheError> {
    scan(path, None, max_bytes, cancelled, |_| {})
}
pub(crate) fn scan(
    path: &Path,
    expected: Option<&AudioProbe>,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
    mut consume: impl FnMut(&[u8]),
) -> Result<(String, u64), WaveformCacheError> {
    let check = || {
        if cancelled() {
            Err(WaveformCacheError::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    if !path.is_absolute() {
        return Err(WaveformCacheError::InvalidProbe);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(WaveformCacheError::InvalidProbe);
    }
    if before.len() > max_bytes {
        return Err(WaveformCacheError::Wav(crate::WavError::TooLarge));
    }
    if expected.is_some_and(|p| p.byte_length != before.len()) {
        return Err(WaveformCacheError::SourceChanged);
    }
    let mut hash = Sha256::new();
    let mut length = 0u64;
    let mut buffer = [0; 65536];
    loop {
        check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        length += n as u64;
        if length > before.len() {
            return Err(WaveformCacheError::SourceChanged);
        }
        hash.update(&buffer[..n]);
        consume(&buffer[..n]);
    }
    let stamp = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let after = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    let hash = format!("{:x}", hash.finalize());
    if !current.is_file()
        || stamp(&before) != stamp(&after)
        || stamp(&after) != stamp(&current)
        || length != before.len()
        || expected.is_some_and(|p| p.sha256 != hash)
    {
        return Err(WaveformCacheError::SourceChanged);
    }
    check()?;
    Ok((hash, length))
}
