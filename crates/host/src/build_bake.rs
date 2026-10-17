//! An explicit no-bake result for directly renderable content. Adding a new
//! clip kind must make an explicit choice here instead of silently skipping it.
use crate::{
    build_record::decode,
    build_state::{BakeOutcome, Receipt, invalid},
    sessions::{Sessions, storage_error},
};
use sungeki_project::{ClipContent, JobPhase};
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn finish(
    sessions: &mut Sessions,
    expected: &StoredJob,
) -> Result<BakeOutcome, ApiError> {
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
    {
        return Err(invalid("親記録が更新済みです"));
    }
    let (input, mut state) = decode(expected)?;
    let (stored, current) = sessions.load(&expected.session_id)?;
    let project = current.history().project();
    let proof = state
        .preflight
        .as_ref()
        .ok_or_else(|| invalid("出力前検査がありません"))?;
    if project.id != input.plan.project_id
        || stored.revision != state.expected_revision
        || stored.revision != proof.revision
        || sungeki_storage::project_hash(project).map_err(invalid)? != proof.content_hash
    {
        return Err(invalid("出力前検査から作品が変更されています"));
    }
    if let Some(outcome) = state.bake {
        return Ok(outcome);
    }
    let index = usize::from(input.speech_count) + 4;
    if state.phase != JobPhase::Running
        || state.receipts.len() != index
        || state.child_id.as_ref() != Some(&input.step_ids[index])
    {
        return Err(invalid("事前描画工程が未予約です"));
    }
    for clip in &project.clips {
        match &clip.content {
            // These primitives/effects are evaluated directly by ScenePlan and
            // the formal image/audio renderer; no surrogate originals are made.
            ClipContent::Vrm { .. }
            | ClipContent::Video { .. }
            | ClipContent::Image { .. }
            | ClipContent::Psd { .. }
            | ClipContent::Sprite { .. }
            | ClipContent::Text { .. }
            | ClipContent::Shape { .. }
            | ClipContent::Blur { .. }
            | ClipContent::Audio { .. }
            | ClipContent::Speech { .. } => {}
        }
    }
    // Reuse supported-content and speech freshness checks, not just enum shape.
    crate::render_api::plan(project)?;
    state.bake = Some(BakeOutcome::NotRequired);
    state.receipts.push(Receipt {
        id: input.step_ids[index].clone(),
        revision: stored.revision,
    });
    state.child_id = None;
    state.validate(&input)?;
    sessions
        .store
        .update_job(
            &expected.id,
            JobKind::BuildVideo,
            expected.version,
            &serde_json::to_vec(&state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    Ok(BakeOutcome::NotRequired)
}
