//! Bounded byte identity check for all collection source kinds. No decoding,
//! copying, environment fallback, or acceptance of leaf symlinks/devices.
use crate::collection::{CollectionError, CollectionSourceStatus as Status};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
fn identity(m: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
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
pub(crate) fn verify(
    path: &Path,
    expected_hash: &str,
    expected_bytes: u64,
    cancelled: &impl Fn() -> bool,
) -> Result<Status, CollectionError> {
    let check = || {
        if cancelled() {
            Err(CollectionError::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    let mut saw_cancel = false;
    let mut read = || -> Result<Status, std::io::Error> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let before = file.metadata()?;
        if !before.is_file() {
            return Ok(Status::Unreadable("通常ファイルではありません".into()));
        }
        if before.len() != expected_bytes {
            return Ok(Status::Changed);
        }
        let mut buffer = [0u8; 65536];
        let mut hash = Sha256::new();
        let mut count = 0u64;
        loop {
            if cancelled() {
                saw_cancel = true;
                return Ok(Status::NotInspected);
            }
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > expected_bytes {
                return Ok(Status::Changed);
            }
            hash.update(&buffer[..n]);
        }
        let after = file.metadata()?;
        let current = fs::symlink_metadata(path)?;
        if !current.is_file()
            || identity(&before) != identity(&after)
            || identity(&after) != identity(&current)
            || count != expected_bytes
            || format!("{:x}", hash.finalize()) != expected_hash
        {
            Ok(Status::Changed)
        } else {
            Ok(Status::Verified)
        }
    };
    let outcome = match read() {
        Ok(status) => status,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Status::Missing,
        Err(e) => Status::Unreadable(e.to_string()),
    };
    if saw_cancel {
        return Err(CollectionError::Cancelled);
    }
    check()?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct Source {
        root: std::path::PathBuf,
        path: std::path::PathBuf,
        bytes: Vec<u8>,
        hash: String,
    }
    impl Source {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("collection-scan-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let path = root.join("source");
            let bytes = vec![42; 200_000];
            fs::write(&path, &bytes).unwrap();
            let hash = format!("{:x}", Sha256::digest(&bytes));
            Self {
                root,
                path,
                bytes,
                hash,
            }
        }
    }
    impl Drop for Source {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn cancellation_between_chunks_is_sticky_even_for_a_one_shot_signal() {
        let f = Source::new();
        let calls = Cell::new(0);
        let result = verify(&f.path, &f.hash, f.bytes.len() as u64, &|| {
            calls.set(calls.get() + 1);
            calls.get() == 3
        });
        assert_eq!(result, Err(CollectionError::Cancelled));
        assert_eq!(fs::read(&f.path).unwrap(), f.bytes);
    }
    #[test]
    fn same_size_change_and_same_content_path_replacement_are_detected() {
        for replacement in [false, true] {
            let f = Source::new();
            let calls = Cell::new(0);
            let result = verify(&f.path, &f.hash, f.bytes.len() as u64, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == 3 {
                    if replacement {
                        fs::rename(&f.path, f.root.join("moved")).unwrap();
                        fs::write(&f.path, &f.bytes).unwrap();
                    } else {
                        let mut changed = f.bytes.clone();
                        changed[100_000] ^= 1;
                        fs::write(&f.path, changed).unwrap();
                    }
                }
                false
            })
            .unwrap();
            assert_eq!(result, Status::Changed);
        }
    }
}
