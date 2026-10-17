//! Explicit, non-recursive cleanup of a settled render's private work files.
//! No Drop cleanup: the host must first durably publish the terminal job state.
use std::{
    ffi::CString,
    fs::{self, File, Metadata, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

pub struct RenderIntermediates {
    directory: File,
    path: PathBuf,
    files: Vec<(CString, Metadata)>,
}
fn changed() -> io::Error {
    io::Error::other("render work files changed; cleanup refused")
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    (a.dev(), a.ino()) == (b.dev(), b.ino())
}
fn numbered(name: &str, prefix: &str, suffix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(suffix))
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}
fn disposable(name: &str) -> bool {
    matches!(name, "font.ttc" | "mix.wav" | "master.wav" | "encoded.wav")
        || numbered(name, "source-", "")
        || numbered(name, "frame-", ".png")
        || numbered(name, "segment-", ".mp4")
        || numbered(name, "decoded-source-", ".wav")
}
impl RenderIntermediates {
    /// Open immediately after creating a new private job directory.
    pub fn new(path: &Path) -> io::Result<Self> {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(path)?;
        if !path.is_absolute() {
            return Err(changed());
        }
        Ok(Self {
            directory,
            path: path.to_owned(),
            files: Vec::new(),
        })
    }
    /// Freeze identities after all workers have exited. Never follow links or
    /// recurse into directories; unknown files and diagnostic JSON are retained.
    pub fn capture(mut self) -> io::Result<Self> {
        let current = fs::symlink_metadata(&self.path)?;
        if !current.is_dir() || !same(&current, &self.directory.metadata()?) {
            return Err(changed());
        }
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str().filter(|n| disposable(n)) else {
                continue;
            };
            let metadata = fs::symlink_metadata(entry.path())?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(changed());
            }
            self.files.push((CString::new(name)?, metadata));
        }
        Ok(self)
    }
    /// Called only after the terminal state (and successful output index) is durable.
    /// Descriptor-relative removal cannot follow a replaced directory path.
    pub fn remove(self) -> io::Result<u64> {
        let mut removed = 0;
        for (name, expected) in &self.files {
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let file = unsafe { File::from_raw_fd(fd) };
            let actual = file.metadata()?;
            let stamp = |m: &Metadata| {
                (
                    m.len(),
                    m.mtime(),
                    m.mtime_nsec(),
                    m.ctime(),
                    m.ctime_nsec(),
                )
            };
            if !same(&actual, expected)
                || stamp(&actual) != stamp(expected)
                || !actual.is_file()
                || actual.nlink() != 1
            {
                return Err(changed());
            }
            if unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error());
            }
            removed += expected.len();
        }
        self.directory.sync_all()?;
        Ok(removed)
    }
}
