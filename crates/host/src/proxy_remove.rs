//! One bounded cache entry; no recursive scans or source access under editor lock.
use crate::{
    image_probe::video::{
        Admission,
        proxy_record::{decode, invalid, record},
    },
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_protocol::{ApiError, ErrorCode, RemoveProxyArgs, SessionResponse};
use sungeki_storage::JobKind;
fn busy() -> ApiError {
    let mut error = ApiError::new(
        ErrorCode::Conflict,
        "軽量動画の生成終了後に削除してください",
    );
    error.retryable = true;
    error
}
pub(super) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: RemoveProxyArgs,
) -> Result<SessionResponse, ApiError> {
    // Keep registration excluded until deletion ends. Admission excludes decoders
    // and publishers, including a worker which has released its slot but has not
    // yet saved the completed output. Active proxy flags cover that latter gap.
    let sessions = shared.lock().map_err(invalid)?;
    let job = record(&sessions, &args.job_id)?;
    let (input, state) = decode(&job)?;
    if !state.phase.is_terminal() {
        return Err(busy());
    }
    if !input.asset_ids.contains(&args.asset_id) {
        return Err(ApiError::new(
            ErrorCode::ValidationFailed,
            "このジョブの対象素材ではありません",
        ));
    }
    let runtime = sessions.render_runtime.clone();
    {
        let flags = runtime.flags.lock().map_err(invalid)?;
        for id in flags.keys() {
            let active = sessions.store.load_job(id).map_err(storage_error)?;
            if active.is_some_and(|j| j.kind == JobKind::Proxy) {
                return Err(busy());
            }
        }
    }
    let admission = Admission::acquire(runtime)?;
    admission
        .runtime
        .video_previews
        .lock()
        .map_err(invalid)?
        .clear();
    let key = input.cache_key(&args.asset_id)?;
    let removed =
        sungeki_assets::proxy_cleanup::remove(&input.cache_root, &key, &admission.runtime.stopping)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::Interrupted {
                    ApiError::new(
                        ErrorCode::Cancelled,
                        "軽量動画の削除を中断しました。同じ対象で再試行できます",
                    )
                } else {
                    invalid(e)
                }
            })?;
    // Immutable job history describes generation, not present cache availability.
    Ok(SessionResponse::ProxyRemoved {
        job_id: args.job_id,
        asset_id: args.asset_id,
        removed,
    })
}
