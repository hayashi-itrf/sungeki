//! Detect original replacement while a read-only preview uses its snapshot.
use crate::image_probe::video::error;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use sungeki_media::{StopReason, video_probe::ProbeError};
use sungeki_protocol::{ApiError, ErrorCode};
#[derive(Debug, PartialEq, Eq)]
pub(super) struct OriginalStamp([u64; 7]);
impl OriginalStamp {
    pub(super) fn read(path: &Path) -> Result<Self, ApiError> {
        let m = fs::symlink_metadata(path).map_err(|e| error(ProbeError::Io(e)))?;
        if !m.is_file() {
            return Err(error(ProbeError::Changed));
        }
        Ok(Self([
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime() as u64,
            m.mtime_nsec() as u64,
            m.ctime() as u64,
            m.ctime_nsec() as u64,
        ]))
    }
    pub(super) fn verify(&self, path: &Path) -> Result<(), ApiError> {
        if Self::read(path)? != *self {
            return Err(error(ProbeError::Changed));
        }
        Ok(())
    }
}
pub(super) fn check(flag: &AtomicBool, end: Instant) -> Result<(), ApiError> {
    let reason = if flag.load(Ordering::SeqCst) {
        Some(StopReason::Cancelled)
    } else if Instant::now() >= end {
        Some(StopReason::Deadline)
    } else {
        None
    };
    reason.map_or(Ok(()), |reason| Err(error(ProbeError::Stopped(reason))))
}
pub(super) fn render_error(
    cause: sungeki_render::RenderError,
    flag: &AtomicBool,
    end: Instant,
) -> ApiError {
    if let Err(stopped) = check(flag, end) {
        return stopped;
    }
    let code = match cause {
        sungeki_render::RenderError::SourceChanged(_) => ErrorCode::ExternalModification,
        _ => ErrorCode::ValidationFailed,
    };
    ApiError::new(code, cause.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_replacement_deletion_and_symlink_are_rejected() {
        let root = std::env::temp_dir().join(format!("preview-stamp-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("source");
        fs::write(&path, b"original").unwrap();
        let stamp = OriginalStamp::read(&path).unwrap();
        stamp.verify(&path).unwrap();
        fs::write(&path, b"changed size").unwrap();
        assert_eq!(
            stamp.verify(&path).unwrap_err().code,
            ErrorCode::ExternalModification
        );
        let stamp = OriginalStamp::read(&path).unwrap();
        let replacement = root.join("replacement");
        fs::write(&replacement, b"replaced").unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert_eq!(
            stamp.verify(&path).unwrap_err().code,
            ErrorCode::ExternalModification
        );
        fs::remove_file(&path).unwrap();
        assert_eq!(
            stamp.verify(&path).unwrap_err().code,
            ErrorCode::AssetMissing
        );
        std::os::unix::fs::symlink("missing", &path).unwrap();
        assert_eq!(
            stamp.verify(&path).unwrap_err().code,
            ErrorCode::ExternalModification
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_and_deadline_keep_stop_reason() {
        let flag = AtomicBool::new(true);
        let end = Instant::now() + std::time::Duration::from_secs(1);
        let e = render_error(
            sungeki_render::RenderError::Message("decode failed".into()),
            &flag,
            end,
        );
        assert_eq!(e.code, ErrorCode::Cancelled);
        assert_eq!(e.details["stop_reason"], "cancelled");
        flag.store(false, Ordering::SeqCst);
        let e = check(&flag, Instant::now()).unwrap_err();
        assert_eq!(e.code, ErrorCode::Cancelled);
        assert_eq!(e.details["stop_reason"], "deadline");
    }
}
