//! Same-parent directory staging; the host must populate and verify the complete
//! collection before commit. No overwrite or recursive deletion of user output.
use crate::{
    PublishError,
    verified_copy::{check_cancel, same_file},
};
use std::{
    ffi::CString,
    fs::{self, File, Metadata},
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::DirBuilderExt},
    },
    path::{Path, PathBuf},
};

pub struct CollectionStage {
    path: PathBuf,
    destination: PathBuf,
    identity: Metadata,
    parent: File,
    parent_identity: Metadata,
    committed: bool,
}
impl CollectionStage {
    pub fn new(destination: &Path) -> Result<Self, PublishError> {
        if !destination.is_absolute()
            || destination.file_name().is_none()
            || destination
                .file_name()
                .is_some_and(|n| n == "." || n == "..")
        {
            return Err(PublishError::InvalidDestination);
        }
        let root = destination
            .parent()
            .ok_or(PublishError::InvalidDestination)?
            .canonicalize()?;
        let destination = root.join(destination.file_name().unwrap());
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(PublishError::OutputExists),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let parent = File::open(&root)?;
        let parent_identity = parent.metadata()?;
        let path = root.join(format!(".vvm-collect-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        let identity = fs::symlink_metadata(&path)?;
        Ok(Self {
            path,
            destination,
            identity,
            parent,
            parent_identity,
            committed: false,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn record(&self) -> Result<crate::CollectionStageRecord, PublishError> {
        if !self.owns() {
            return Err(PublishError::Changed);
        }
        self.parent.sync_all()?;
        crate::CollectionStageRecord::new(
            self.path.clone(),
            self.destination.clone(),
            &self.identity,
            &self.parent_identity,
        )
    }
    fn owns(&self) -> bool {
        self.path.parent().is_some_and(|p| {
            fs::symlink_metadata(p)
                .is_ok_and(|m| m.is_dir() && same_file(&self.parent_identity, &m))
        }) && fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.is_dir() && same_file(&self.identity, &m))
    }
    /// The private tree must already contain its verified document, manifest and
    /// assets. Cancellation ends before rename; a visible complete result is not
    /// subsequently reported as cancelled.
    pub fn commit(mut self, mut cancelled: impl FnMut() -> bool) -> Result<PathBuf, PublishError> {
        check_cancel(&mut cancelled)?;
        if !self.owns() {
            return Err(PublishError::Changed);
        }
        for name in ["project.vvm.json", "manifest.json"] {
            if !fs::symlink_metadata(self.path.join(name))?.is_file() {
                return Err(PublishError::Changed);
            }
        }
        if !fs::symlink_metadata(self.path.join("assets"))?.is_dir() {
            return Err(PublishError::Changed);
        }
        crate::collection_tree::sync(&self.path, &mut cancelled)?;
        check_cancel(&mut cancelled)?;
        if !self.owns() {
            return Err(PublishError::Changed);
        }
        let from = CString::new(self.path.file_name().unwrap().as_bytes())
            .map_err(|_| PublishError::InvalidDestination)?;
        let to = CString::new(self.destination.file_name().unwrap().as_bytes())
            .map_err(|_| PublishError::InvalidDestination)?;
        rename_exclusive(self.parent.as_raw_fd(), &from, &to)?;
        self.committed = true;
        self.parent
            .sync_all()
            .map_err(|source| PublishError::DurabilityUncertain {
                path: self.destination.clone(),
                source,
            })?;
        Ok(self.destination.clone())
    }
}
impl Drop for CollectionStage {
    fn drop(&mut self) {
        if !self.committed && self.owns() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
fn rename_exclusive(
    parent: std::os::fd::RawFd,
    from: &CString,
    to: &CString,
) -> Result<(), PublishError> {
    #[cfg(target_os = "macos")]
    let status = unsafe {
        libc::renameatx_np(
            parent,
            from.as_ptr(),
            parent,
            to.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(target_os = "linux")]
    let status = unsafe {
        libc::renameat2(
            parent,
            from.as_ptr(),
            parent,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let status = {
        let _ = (parent, from, to);
        return Err(PublishError::InvalidDestination);
    };
    if status == 0 {
        return Ok(());
    }
    let e = std::io::Error::last_os_error();
    if e.kind() == std::io::ErrorKind::AlreadyExists {
        Err(PublishError::OutputExists)
    } else {
        Err(e.into())
    }
}
