//! Read-only import inspection outside the editor lock, sharing media admission.
#[path = "video_probe_cancel.rs"]
pub(crate) mod cancellation;
#[path = "proxy_api.rs"]
pub(crate) mod proxy_api;
#[path = "proxy_execute.rs"]
mod proxy_execute;
#[path = "proxy_record.rs"]
pub(crate) mod proxy_record;
#[path = "proxy_worker.rs"]
pub(crate) mod proxy_worker;
#[path = "vrm_import.rs"]
pub(crate) mod vrm_import;
use crate::{media_job_runtime::Runtime, sessions::Sessions};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_media::{
    StopReason,
    video_probe::{ProbeError, ProbeRuntime},
};
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};
pub(crate) struct Admission {
    pub(crate) runtime: Runtime,
    _slot: tokio::sync::OwnedSemaphorePermit,
}
impl Admission {
    pub(crate) fn acquire(runtime: Runtime) -> Result<Self, ApiError> {
        let flags = runtime.flags.lock().map_err(ApiError::io)?;
        if runtime.stopping.load(Ordering::SeqCst) {
            return Err(ApiError::new(ErrorCode::Cancelled, "終了処理中です"));
        }
        let slot = runtime.slots.clone().try_acquire_owned().map_err(|_| {
            let mut e = ApiError::new(
                ErrorCode::Conflict,
                "別の素材処理が実行中です。後で再試行してください",
            );
            e.retryable = true;
            e
        })?;
        runtime.active.fetch_add(1, Ordering::SeqCst);
        drop(flags);
        Ok(Self {
            runtime,
            _slot: slot,
        })
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        self.runtime.active.fetch_sub(1, Ordering::SeqCst);
        self.runtime.wake.notify_waiters();
    }
}
pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    path: String,
) -> Result<SessionResponse, ApiError> {
    execute_cancellable(shared, path, || false)
}
pub(crate) fn execute_cancellable(
    shared: &Arc<Mutex<Sessions>>,
    path: String,
    cancelled: impl FnMut() -> bool,
) -> Result<SessionResponse, ApiError> {
    if !Path::new(&path).is_absolute() || path.contains('\0') {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "動画原本の絶対パスを指定してください",
        ));
    }
    let (runtime, database) = {
        let sessions = shared.lock().map_err(ApiError::io)?;
        (
            sessions.render_runtime.clone(),
            sessions.review_database_path.clone(),
        )
    };
    let admission = Admission::acquire(runtime)?;
    let saved = crate::runtime_settings::load(&database)?;
    let ffprobe = crate::runtime_settings::file(
        std::env::var_os("SUNGEKI_RENDER_FFPROBE").map(PathBuf::from),
        saved.ffprobe.as_deref(),
        "FFprobe",
    )?;
    let root = crate::render_location::root(&database)?;
    let probe = cancellation::run(&admission.runtime.stopping, cancelled, |flag| {
        sungeki_media::video_probe::probe(
            Path::new(&path),
            &ProbeRuntime {
                ffprobe: &ffprobe,
                root: &root,
                timeout: Duration::from_secs(55),
                max_source_bytes: 4 * 1024 * 1024 * 1024,
            },
            flag,
        )
        .map_err(error)
    })?;
    if admission.runtime.stopping.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            ErrorCode::Cancelled,
            "動画検査を中断しました",
        ));
    }
    Ok(SessionResponse::VideoProbed { probe })
}
pub(crate) fn error(cause: ProbeError) -> ApiError {
    let code = match &cause {
        ProbeError::Changed => ErrorCode::ExternalModification,
        ProbeError::Stopped(StopReason::Cancelled | StopReason::Deadline) => ErrorCode::Cancelled,
        ProbeError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => ErrorCode::AssetMissing,
        ProbeError::Io(e) if e.kind() == std::io::ErrorKind::Interrupted => ErrorCode::Cancelled,
        ProbeError::Io(_) => ErrorCode::IoError,
        _ => ErrorCode::ValidationFailed,
    };
    let mut result = ApiError::new(code, cause.to_string());
    if let ProbeError::Stopped(reason) = cause {
        result
            .details
            .insert("stop_reason".into(), format!("{reason:?}").to_lowercase());
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn probe_admission_is_counted_and_settled_before_shutdown() {
        let runtime = Runtime::default();
        let admitted = Admission::acquire(runtime.clone()).unwrap();
        assert_eq!(runtime.active.load(Ordering::SeqCst), 1);
        let busy = Admission::acquire(runtime.clone()).err().unwrap();
        assert_eq!(busy.code, ErrorCode::Conflict);
        assert!(busy.retryable);
        let shutdown = runtime.clone();
        let task = tokio::spawn(async move {
            shutdown.shutdown().await;
        });
        tokio::task::yield_now().await;
        assert!(runtime.stopping.load(Ordering::SeqCst));
        assert!(!task.is_finished());
        assert_eq!(
            Admission::acquire(runtime.clone()).err().unwrap().code,
            ErrorCode::Cancelled
        );
        drop(admitted);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(runtime.active.load(Ordering::SeqCst), 0);
        assert_eq!(runtime.slots.available_permits(), 1);
    }
    #[test]
    fn public_failures_distinguish_changed_missing_invalid_and_cancelled() {
        assert_eq!(
            error(ProbeError::Changed).code,
            ErrorCode::ExternalModification
        );
        assert_eq!(
            error(ProbeError::Io(std::io::ErrorKind::NotFound.into())).code,
            ErrorCode::AssetMissing
        );
        assert_eq!(
            error(ProbeError::Invalid("bad")).code,
            ErrorCode::ValidationFailed
        );
        let deadline = error(ProbeError::Stopped(StopReason::Deadline));
        assert_eq!(deadline.code, ErrorCode::Cancelled);
        assert_eq!(deadline.details["stop_reason"], "deadline");
    }
}

#[cfg(test)]
#[path = "proxy_roundtrip_tests.rs"]
mod proxy_roundtrip;
