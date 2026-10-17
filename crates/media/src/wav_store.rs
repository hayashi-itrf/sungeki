use crate::VerifiedWav;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct WavStoreError {
    pub source: std::io::Error,
    /// true means destination may already exist; reconcile it instead of retrying
    /// with overwrite or deleting it. It is not a confirmed durable success.
    pub published: bool,
}
impl std::fmt::Display for WavStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "音声の保存に失敗しました（確定先作成済み: {}）: {}",
            self.published, self.source
        )
    }
}
impl std::error::Error for WavStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Publish NEW verified bytes in an existing private job directory. The owner
/// reserves that directory and must not mutate it concurrently. No overwrite.
/// The hard link publishes our own synced temporary inode, never a mutable input.
/// Filesystems without hard-link support fail safely; no overwrite fallback.
pub fn store_wav_new(audio: &VerifiedWav, destination: &Path) -> Result<(), WavStoreError> {
    let mut published = false;
    let result = (|| -> std::io::Result<()> {
        if !destination.is_absolute() || destination.file_name().is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "音声保存先には絶対ファイルパスが必要です",
            ));
        }
        let parent = destination.parent().expect("absolute file parent");
        let temporary = parent.join(format!(".vvm-audio-{}.tmp", uuid::Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        // Install cleanup only after create_new succeeds: never remove another file.
        let guard = Temporary(temporary);
        file.write_all(audio.bytes())?;
        file.sync_all()?;
        drop(file);
        fs::hard_link(&guard.0, destination)?;
        published = true;
        File::open(parent)?.sync_all()?;
        fs::remove_file(&guard.0)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    result.map_err(|source| WavStoreError { source, published })
}
