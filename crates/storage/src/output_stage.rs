//! Shared atomic, no-clobber publication for video and subtitle outputs.
use crate::publish_output::{PublishError, PublishedOutput};
use crate::verified_copy::{check_cancel, same_file, sync_directory, verified_copy};
use std::fs::{self, File, Metadata, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(crate) struct Stage {
    path: PathBuf,
    identity: Metadata,
}

impl Stage {
    pub(crate) fn new(parent: &Path) -> Result<(Self, File), PublishError> {
        let path = parent.join(format!(".vvm-output-{}.tmp", uuid::Uuid::new_v4()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let identity = file.metadata()?;
        Ok((Self { path, identity }, file))
    }

    fn owns(&self, path: &Path) -> bool {
        fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && same_file(&self.identity, &metadata))
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if self.owns(&self.path) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn destination(path: &Path, extensions: &[&str]) -> Result<PathBuf, PublishError> {
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|e| extensions.iter().any(|ext| e.eq_ignore_ascii_case(ext)))
        || path.file_name().is_none()
    {
        return Err(PublishError::InvalidDestination);
    }
    let parent = path
        .parent()
        .ok_or(PublishError::InvalidDestination)?
        .canonicalize()?;
    Ok(parent.join(path.file_name().unwrap()))
}

pub(crate) fn commit(
    stage: Stage,
    file: File,
    path: PathBuf,
    sha256: &str,
    byte_length: u64,
    original: Option<&File>,
    mut cancelled: impl FnMut() -> bool,
) -> Result<PublishedOutput, PublishError> {
    let parent = path.parent().unwrap();
    file.sync_all()?;
    check_cancel(&mut cancelled)?;
    if !stage.owns(&stage.path) {
        return Err(PublishError::Changed);
    }
    let already_present = match fs::hard_link(&stage.path, &path) {
        Ok(()) => {
            if !stage.owns(&path) {
                return Err(PublishError::Changed);
            }
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            match verified_copy(
                &path,
                sha256,
                byte_length,
                &mut std::io::sink(),
                &mut cancelled,
            ) {
                Ok(existing) => {
                    if let Some(original) = original
                        && same_file(&original.metadata()?, &existing.metadata()?)
                    {
                        return Err(PublishError::OutputExists);
                    }
                    existing
                        .sync_all()
                        .map_err(|source| PublishError::DurabilityUncertain {
                            path: path.clone(),
                            source,
                        })?;
                    true
                }
                Err(PublishError::Cancelled) => return Err(PublishError::Cancelled),
                Err(_) => return Err(PublishError::OutputExists),
            }
        }
        Err(error) => return Err(error.into()),
    };
    // No cancellation after commit: callers must not report the visible output as cancelled.
    drop(file);
    drop(stage);
    sync_directory(parent).map_err(|source| PublishError::DurabilityUncertain {
        path: path.clone(),
        source,
    })?;
    Ok(PublishedOutput {
        path,
        already_present,
    })
}
