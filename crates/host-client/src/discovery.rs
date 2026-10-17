use std::{
    env, fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::PathBuf,
};
use sungeki_protocol::{ApiError, ErrorCode, SOCKET_NAME};

#[derive(Clone, Debug)]
pub struct HostOptions {
    pub runtime_dir: PathBuf,
    pub host_binary: PathBuf,
}

impl HostOptions {
    pub fn from_environment() -> Result<Self, ApiError> {
        // SAFETY: geteuid has no arguments or failure mode.
        let uid = unsafe { libc::geteuid() };
        let runtime_dir = env::var_os("SUNGEKI_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/sungeki-{uid}")));
        let host_binary = match env::var_os("SUNGEKI_HOST_BINARY") {
            Some(path) => PathBuf::from(path),
            None => env::current_exe()
                .map_err(ApiError::io)?
                .parent()
                .ok_or_else(|| {
                    ApiError::new(
                        ErrorCode::SidecarMissing,
                        "実行ファイルの位置を取得できません",
                    )
                })?
                .join("sungeki-host"),
        };
        let options = Self {
            runtime_dir,
            host_binary,
        };
        options.validate()?;
        Ok(options)
    }

    pub fn validate(&self) -> Result<(), ApiError> {
        if !self.runtime_dir.is_absolute() || self.runtime_dir.as_os_str().len() > 85 {
            return Err(ApiError::new(
                ErrorCode::InvalidRuntime,
                "runtime領域は85バイト以内の絶対パスが必要です",
            ));
        }
        if !self.host_binary.is_absolute() {
            return Err(ApiError::new(
                ErrorCode::SidecarMissing,
                "host実行ファイルは絶対パスが必要です",
            ));
        }
        match fs::symlink_metadata(&self.runtime_dir) {
            Ok(metadata) => {
                // SAFETY: geteuid has no arguments or failure mode.
                let uid = unsafe { libc::geteuid() };
                if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
                    return Err(ApiError::new(
                        ErrorCode::InvalidRuntime,
                        "runtime領域の所有者/権限が不正です",
                    ));
                }
                match fs::symlink_metadata(self.socket()) {
                    Ok(socket) if !socket.file_type().is_socket() || socket.uid() != uid => {
                        return Err(ApiError::new(
                            ErrorCode::InvalidRuntime,
                            "接続先が所有するUnixソケットではありません",
                        ));
                    }
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                        return Err(ApiError::io(error));
                    }
                    _ => {}
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ApiError::io(error)),
        }
        Ok(())
    }

    pub fn socket(&self) -> PathBuf {
        self.runtime_dir.join(SOCKET_NAME)
    }
}
