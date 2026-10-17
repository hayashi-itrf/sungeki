#[path = "build_api.rs"]
mod api;
#[path = "build_continuation.rs"]
pub(crate) mod continuation;
#[path = "build_reuse_reconcile.rs"]
pub(crate) mod reuse_reconcile;
#[path = "build_reuse_runtime.rs"]
pub(crate) mod reuse_runtime;
#[path = "build_reuse_target.rs"]
pub(crate) mod reuse_target;
#[path = "build_reused_child.rs"]
pub(crate) mod reused_child;
#[path = "build_reused_original.rs"]
pub(crate) mod reused_original;
#[path = "build_reused_recovery.rs"]
pub(crate) mod reused_recovery;
#[path = "build_settlement.rs"]
pub(crate) mod settlement;
#[path = "build_speech_reuse.rs"]
pub(crate) mod speech_reuse;
pub(crate) use api::execute;
#[path = "build_engine.rs"]
mod engine;
pub(crate) use engine::{SpeechConnection, profiles};
#[path = "build_shutdown.rs"]
mod shutdown;
pub(crate) use shutdown::shutdown;
#[path = "build_cancel_monitor.rs"]
mod cancel_monitor;
pub(crate) use cancel_monitor::launch as monitor_cancel;
#[path = "build_settle.rs"]
mod settle;
pub(crate) use settle::settle as settle_cancel;
#[path = "build_cancel.rs"]
mod cancel;
pub(crate) use cancel::request as cancel_owned;
#[path = "build_runtime.rs"]
mod runtime;
pub(crate) use runtime::{Runtime, launch};
#[path = "build_dispatch.rs"]
mod dispatch;
pub(crate) use dispatch::{Advance, advance};
#[path = "build_prepare.rs"]
mod prepare;
#[path = "build_presets.rs"]
mod presets;
pub(crate) use prepare::prepare;
#[path = "build_sources.rs"]
mod sources;
pub(crate) use sources::verify as verify_sources;
#[path = "build_create.rs"]
mod create;
pub(crate) use create::{create, create_bound, create_configured, create_continued};
#[path = "build_review_result.rs"]
mod review_result;
pub(crate) use review_result::accept as accept_review;
#[path = "build_review.rs"]
mod review;
pub(crate) use review::{start as start_review, validate_parent as validate_review_parent};
#[path = "build_render_result.rs"]
mod render_result;
pub(crate) use render_result::accept as accept_render;
#[path = "build_render.rs"]
mod render;
pub(crate) use render::{start as start_render, validate_parent as validate_render_parent};
#[path = "build_save.rs"]
mod save;
pub(crate) use save::save as save_project;
#[path = "build_bake.rs"]
mod bake;
pub(crate) use bake::finish as finish_bake;
#[path = "build_preflight.rs"]
mod preflight;
pub(crate) use preflight::validate as validate_output;
#[path = "build_layout_apply.rs"]
mod layout_apply;
pub(crate) use layout_apply::layout;
#[path = "build_speech_apply.rs"]
mod speech_apply;
pub(crate) use speech_apply::apply as apply_speech;
#[path = "build_speech_input.rs"]
mod speech_input;
pub(crate) use speech_input::{start as start_speech, validate_parent as validate_speech_parent};
#[path = "build_reservation.rs"]
mod reservation;
pub(crate) use reservation::{Reservation, request_cancel, reserve};
#[path = "build_compose.rs"]
mod compose;
use crate::{
    build_state::{Input, State, invalid},
    sessions::{Sessions, storage_error},
};
pub(crate) use compose::compose;
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn decode(job: &StoredJob) -> Result<(Input, State), ApiError> {
    if job.kind != JobKind::BuildVideo {
        return Err(invalid("種類が不一致です"));
    }
    let input: Input = serde_json::from_slice(&job.input).map_err(invalid)?;
    let state: State = serde_json::from_slice(&job.state).map_err(invalid)?;
    state.validate(&input)?;
    Ok((input, state))
}
/// No child is dispatched, cancelled, or marked settled by recovery. Existing
/// child journals/resource recovery retain responsibility for external processes.
pub(crate) fn recover(sessions: &mut Sessions) -> Result<(), ApiError> {
    let mut after = None;
    loop {
        let ids = sessions
            .store
            .list_jobs(Some(JobKind::BuildVideo), None, after.as_deref(), 100)
            .map_err(storage_error)?;
        if ids.is_empty() {
            return Ok(());
        }
        for id in &ids {
            let job = sessions
                .store
                .load_job(id)
                .map_err(storage_error)?
                .ok_or_else(|| invalid("記録がありません"))?;
            let (input, mut state) = decode(&job)?;
            if !state.phase.is_terminal() {
                state.phase = JobPhase::Interrupted;
                state.error = Some(invalid(
                    "hostが中断しました。子処理の状態を確認して明示的に続行してください。自動再実行はしていません",
                ));
                state.validate(&input)?;
                sessions
                    .store
                    .update_job(
                        id,
                        JobKind::BuildVideo,
                        job.version,
                        &serde_json::to_vec(&state).map_err(invalid)?,
                    )
                    .map_err(storage_error)?;
            }
        }
        after = ids.last().cloned();
    }
}

#[cfg(test)]
#[path = "build_record_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "build_managed_integration_tests.rs"]
pub(crate) mod managed_integration_tests;
