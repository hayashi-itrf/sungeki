//! Explicit removal of one cache key. Caller must exclude publication/readers.
//! No recursive deletion and no access to source asset locators.
use crate::proxy::ProxyKey;
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn cancelled(flag: &AtomicBool) -> io::Result<()> {
    if flag.load(Ordering::Acquire) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "cache removal cancelled",
        ))
    } else {
        Ok(())
    }
}
fn open_directory(parent: &File, name: &CString) -> io::Result<File> {
    // Relative descriptor access prevents an ancestor rename from redirecting deletion.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn stat(parent: &File, name: &CString) -> io::Result<libc::stat> {
    let mut value = std::mem::MaybeUninit::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            value.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { value.assume_init() })
}
/// Removes completed, corrupt or interrupted entries using a key derived by host.
/// Unknown files/subdirectories/symlinks reject the entire initial scan. A failure
/// or cancellation after deletion starts may leave a partial entry; retry is safe.
/// Returns false when absent, true when the directory was removed.
pub fn remove(root: &Path, key: &ProxyKey, cancel: &AtomicBool) -> io::Result<bool> {
    cancelled(cancel)?;
    if !root.is_absolute() {
        return Err(invalid("absolute cache root required"));
    }
    let root_file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let name = CString::new(key.as_str()).map_err(|_| invalid("invalid cache key"))?;
    let directory = match open_directory(&root_file, &name) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let identity = directory.metadata()?;
    let same_directory = || -> io::Result<()> {
        let now = stat(&root_file, &name)?;
        if now.st_dev as u64 != identity.dev() || now.st_ino as u64 != identity.ino() {
            return Err(invalid("cache directory replaced"));
        }
        Ok(())
    };
    same_directory()?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(root.join(key.as_str()))? {
        cancelled(cancel)?;
        let name = entry?.file_name();
        if !matches!(
            name.to_str(),
            Some("video.mov" | "manifest.json" | "manifest.pending")
        ) {
            return Err(invalid("unknown file in cache directory"));
        }
        let name = CString::new(name.as_bytes()).map_err(|_| invalid("invalid filename"))?;
        let metadata = stat(&directory, &name)?;
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(invalid("non-regular file in cache directory"));
        }
        entries.push((name, metadata.st_dev, metadata.st_ino));
    }
    same_directory()?;
    // Validate all entries before the first mutation; descriptors stay anchored.
    for (name, device, inode) in &entries {
        cancelled(cancel)?;
        let now = stat(&directory, name)?;
        if now.st_dev != *device
            || now.st_ino != *inode
            || now.st_mode & libc::S_IFMT != libc::S_IFREG
        {
            return Err(invalid("cache file replaced"));
        }
    }
    for (entry, _, _) in entries {
        cancelled(cancel)?;
        if unsafe { libc::unlinkat(directory.as_raw_fd(), entry.as_ptr(), 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    directory.sync_all()?;
    cancelled(cancel)?;
    same_directory()?;
    if unsafe { libc::unlinkat(root_file.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err(io::Error::last_os_error());
    }
    root_file.sync_all()?;
    Ok(true)
}
#[cfg(test)]
#[path = "proxy_cleanup_tests.rs"]
mod tests;
