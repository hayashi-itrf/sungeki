//! Durable ownership evidence, not permission to delete a directory by its name.
use crate::PublishError;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    device: u64,
    inode: u64,
    born_ns: Option<u128>,
}
impl Identity {
    fn of(meta: &fs::Metadata) -> Self {
        Self {
            device: meta.dev(),
            inode: meta.ino(),
            born_ns: meta
                .created()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|time| time.as_nanos()),
        }
    }
    fn matches(&self, meta: &fs::Metadata) -> bool {
        meta.is_dir() && *self == Self::of(meta)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionStageRecord {
    path: PathBuf,
    destination: PathBuf,
    directory: Identity,
    parent: Identity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionStagePresence {
    Owned,
    Missing,
    Changed,
}
impl CollectionStageRecord {
    pub(crate) fn new(
        path: PathBuf,
        destination: PathBuf,
        directory: &fs::Metadata,
        parent: &fs::Metadata,
    ) -> Result<Self, PublishError> {
        let result = Self {
            path,
            destination,
            directory: Identity::of(directory),
            parent: Identity::of(parent),
        };
        result.validate()?;
        Ok(result)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn validate(&self) -> Result<(), PublishError> {
        let valid_name = self
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix(".vvm-collect-"))
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s));
        let clean = |path: &Path| {
            path.is_absolute()
                && path.components().all(|c| {
                    matches!(
                        c,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )
                })
        };
        if !clean(&self.path)
            || !clean(&self.destination)
            || !valid_name
            || self.path == self.destination
            || self.destination.file_name().is_none()
            || self.path.parent() != self.destination.parent()
        {
            return Err(PublishError::InvalidDestination);
        }
        Ok(())
    }
    /// Read-only; Missing also covers a successfully published/renamed stage.
    pub fn inspect(&self) -> Result<CollectionStagePresence, PublishError> {
        self.validate()?;
        let parent = self.path.parent().ok_or(PublishError::InvalidDestination)?;
        let parent_before = match fs::symlink_metadata(parent) {
            Ok(meta) if self.parent.matches(&meta) => meta,
            Ok(_) => return Ok(CollectionStagePresence::Changed),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CollectionStagePresence::Changed);
            }
            Err(e) => return Err(e.into()),
        };
        let result = match fs::symlink_metadata(&self.path) {
            Ok(meta) if self.directory.matches(&meta) => CollectionStagePresence::Owned,
            Ok(_) => CollectionStagePresence::Changed,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => CollectionStagePresence::Missing,
            Err(e) => return Err(e.into()),
        };
        if !Identity::of(&parent_before).matches(&fs::symlink_metadata(parent)?) {
            return Ok(CollectionStagePresence::Changed);
        }
        Ok(result)
    }
}
