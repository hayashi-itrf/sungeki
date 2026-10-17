use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};
use sungeki_protocol::SOCKET_NAME;
use tokio::net::UnixListener;

pub struct Endpoint {
    pub listener: UnixListener,
    socket: PathBuf,
    inode: u64,
    // Lock must outlive socket removal. Never unlink a lock file in use.
    _lock: File,
}

// Reserve ownership before recovery; expose the socket only once ready.
pub struct Reservation {
    socket: PathBuf,
    lock: File,
}

impl Reservation {
    pub fn acquire(directory: &Path) -> io::Result<Option<Self>> {
        if !directory.is_absolute() || directory.as_os_str().len() > 85 {
            return Err(io::Error::other(
                "runtimeパスは短い絶対パスで指定してください",
            ));
        }
        match DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(directory)?;
        // SAFETY: geteuid has no arguments or failure mode.
        let uid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
            return Err(io::Error::other(
                "runtime領域の所有者または権限が不正です（0700が必要）",
            ));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("host.lock"))?;
        let metadata = lock.metadata()?;
        if !metadata.is_file() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o600 {
            return Err(io::Error::other("ホストロックの所有者または権限が不正です"));
        }
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
            Err(std::fs::TryLockError::Error(error)) => return Err(error),
        }
        Ok(Some(Self {
            socket: directory.join(SOCKET_NAME),
            lock,
        }))
    }

    pub fn bind(self) -> io::Result<Endpoint> {
        let socket = self.socket;
        // SAFETY: geteuid has no arguments or failure mode.
        let uid = unsafe { libc::geteuid() };
        match fs::symlink_metadata(&socket) {
            Ok(metadata) if metadata.file_type().is_socket() && metadata.uid() == uid => {
                fs::remove_file(&socket)?;
            }
            Ok(_) => return Err(io::Error::other("ソケット位置に別のファイルがあります")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(&socket)?;
        let inode = fs::symlink_metadata(&socket)?.ino();
        let endpoint = Endpoint {
            listener,
            socket,
            inode,
            _lock: self.lock,
        };
        fs::set_permissions(&endpoint.socket, fs::Permissions::from_mode(0o600))?;
        Ok(endpoint)
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::symlink_metadata(&self.socket) {
            if metadata.file_type().is_socket() && metadata.ino() == self.inode {
                let _ = fs::remove_file(&self.socket);
            }
        }
    }
}
