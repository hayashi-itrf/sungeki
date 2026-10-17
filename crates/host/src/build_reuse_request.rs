//! The synchronous session API runs on the host's blocking request worker.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_protocol::{ApiError, ReconcileBuildAudioArgs};

pub(crate) fn execute(
    shared: &Arc<Mutex<Sessions>>,
    args: ReconcileBuildAudioArgs,
) -> Result<(), ApiError> {
    let runtime = tokio::runtime::Handle::try_current().map_err(invalid)?;
    runtime.block_on(async {
        let (parent, applied) = {
            let sessions = shared.lock().map_err(invalid)?;
            let parent = sessions
                .store
                .load_job(&args.job_id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("復旧する制作がありません"))?;
            let (_, state) = build::decode(&parent)?;
            let child = sessions
                .store
                .load_job(&args.child_job_id)
                .map_err(storage_error)?;
            let applied = if let Some(child) = child {
                let (_, output) = crate::speech_journal::decode_input(&child)?;
                if output.execution != crate::speech_state::Execution::Reused {
                    return Err(invalid("再利用音声の工程だけを復旧できます"));
                }
                matches!(
                    output.application,
                    crate::speech_state::Application::Applied { .. }
                )
            } else {
                false
            };
            if !applied && state.child_id.as_deref() != Some(args.child_job_id.as_str()) {
                return Err(invalid("指定した音声工程は復旧対象ではありません"));
            }
            (parent, applied)
        };
        if applied {
            build::apply_speech(shared, &args.job_id, &args.child_job_id).await?;
        } else {
            build::reuse_reconcile::run(shared, &parent).await?;
        }
        Ok(())
    })
}
