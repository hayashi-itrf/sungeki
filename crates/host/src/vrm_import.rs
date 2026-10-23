//! Read-only metadata inspection. Registration remains a common editor command.
use crate::sessions::Sessions;
use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use sungeki_media::{
    StopReason,
    vrm_probe::{VrmProbeConfig, VrmProbeError, probe_vrm},
};
use sungeki_project::VrmKind;
use sungeki_protocol::{ApiError, ErrorCode, SessionResponse};

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    path: String,
    kind: VrmKind,
) -> Result<SessionResponse, ApiError> {
    if !Path::new(&path).is_absolute() || path.contains('\0') {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "VRM/VRMA原本の絶対パスを指定してください",
        ));
    }
    let (runtime, database) = {
        let sessions = shared.lock().map_err(ApiError::io)?;
        (
            sessions.render_runtime.clone(),
            sessions.review_database_path.clone(),
        )
    };
    let admission = super::Admission::acquire(runtime)?;
    let saved = crate::runtime_settings::load(&database)?;
    let node = crate::render_runtime::bundled::file(
        std::env::var_os("SUNGEKI_VRM_NODE"),
        saved.vrm_node.as_deref(),
        "bin/node",
    )?;
    let worker = crate::render_runtime::bundled::file(
        std::env::var_os("SUNGEKI_VRM_INSPECT_WORKER"),
        saved.vrm_inspect_worker.as_deref(),
        "workers/vrm/src/inspect.ts",
    )?;
    let directory = crate::render_location::root(&database)?;
    let result = probe_vrm(
        &VrmProbeConfig {
            node: &node,
            worker: &worker,
            directory: &directory,
            timeout: Duration::from_secs(15),
        },
        Path::new(&path),
        kind,
        &admission.runtime.stopping,
    )
    .map_err(|error| match error {
        VrmProbeError::Stopped(StopReason::Cancelled) => {
            ApiError::new(ErrorCode::Cancelled, "VRM検査を中断しました")
        }
        VrmProbeError::Stopped(_) => ApiError::new(ErrorCode::IoError, "VRM検査の制限を超えました"),
        VrmProbeError::Invalid(message) => ApiError::new(ErrorCode::ValidationFailed, message),
        error => ApiError::io(error),
    })?;
    if admission.runtime.stopping.load(Ordering::SeqCst) {
        return Err(ApiError::new(ErrorCode::Cancelled, "VRM検査を中断しました"));
    }
    Ok(SessionResponse::VrmProbed {
        probe: result.asset_probe(),
    })
}
