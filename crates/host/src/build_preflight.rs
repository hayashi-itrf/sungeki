//! Output readiness and source-byte checks outside the session lock. These are
//! not a rendered-quality report and are rechecked during formal rendering.
use crate::{
    build_record::decode,
    build_state::{Preflight, Receipt, invalid},
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

pub(crate) async fn validate(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<Preflight, ApiError> {
    let (input, mut state, project, version) = {
        let sessions = shared.lock().map_err(invalid)?;
        if sessions
            .store
            .load_job(&expected.id)
            .map_err(storage_error)?
            .as_ref()
            != Some(expected)
        {
            return Err(invalid("親記録が更新済みです"));
        }
        let (input, state) = decode(expected)?;
        let (stored, current) = sessions.load(&expected.session_id)?;
        if current.history().project().id != input.plan.project_id
            || stored.revision != state.expected_revision
        {
            return Err(invalid("制作対象が変更されています"));
        }
        if let Some(proof) = &state.preflight {
            if sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
                != proof.content_hash
            {
                return Err(invalid("検査した作品から変更されています"));
            }
            return Ok(proof.clone()); // Stored observation, not a new source scan.
        }
        let index = usize::from(input.speech_count) + 3;
        if state.phase != JobPhase::Running
            || state.receipts.len() != index
            || state.child_id.as_ref() != Some(&input.step_ids[index])
        {
            return Err(invalid("出力前検査が未予約です"));
        }
        (
            input,
            state,
            current.history().project().clone(),
            stored.version,
        )
    };
    let revision = state.expected_revision;
    let proof = tokio::task::spawn_blocking(move || {
        let plan = crate::render_api::plan(&project)?;
        let checked_sources = plan.verify_original_hashes().map_err(invalid)?;
        Ok::<_, ApiError>(Preflight {
            revision,
            content_hash: sungeki_storage::project_hash(&project).map_err(invalid)?,
            frame_count: plan.frame_count(),
            checked_sources,
        })
    })
    .await
    .map_err(invalid)??;
    let mut sessions = shared.lock().map_err(invalid)?;
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
        || sessions.load(&expected.session_id)?.0.version != version
    {
        return Err(invalid("検査中に制作状態が変更されました"));
    }
    state.receipts.push(Receipt {
        id: input.step_ids[state.receipts.len()].clone(),
        revision,
    });
    state.child_id = None;
    state.preflight = Some(proof.clone());
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
    Ok(proof)
}
