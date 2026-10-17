//! Atomically apply the reserved Compose step and advance its parent receipt.
use crate::{
    build_record::decode,
    build_state::{Receipt, invalid},
    sessions::{Sessions, edit_error, preflight, storage_error},
};
use sungeki_editor::{EditAction, EditReceipt, EditRequest, plan_build_script};
use sungeki_project::{JobPhase, Project};
use sungeki_protocol::{ApiError, SessionEventKind, SessionResponse};
use sungeki_storage::{StoredJob, StoredSession};

pub(crate) fn compose(
    sessions: &mut Sessions,
    record: &StoredJob,
) -> Result<EditReceipt, ApiError> {
    let (input, mut state) = decode(record)?;
    let (stored, mut candidate) = sessions.load(&record.session_id)?;
    if candidate.history().project().id != input.plan.project_id || state.receipts.is_empty() {
        return Err(invalid("制作先または準備工程が一致しません"));
    }
    let base = Project::empty(input.plan.project_id.clone(), input.recipe.title.clone());
    let request = EditRequest {
        operation_id: input.step_ids[1].clone(),
        expected_revision: state.receipts[0].revision,
        action: EditAction::Apply {
            commands: plan_build_script(&base, &input.recipe, &input.plan.script)
                .map_err(invalid)?,
        },
    };
    if state.receipts.len() > 1 {
        if candidate.recorded_request(&request.operation_id) != Some(&request) {
            return Err(invalid("構成済みの編集記録が一致しません"));
        }
        let receipt = candidate.execute(request).map_err(edit_error)?;
        if receipt.revision != state.receipts[1].revision {
            return Err(invalid("構成済みrevisionが一致しません"));
        }
        return Ok(receipt);
    }
    if state.phase != JobPhase::Running
        || state.child_id.as_ref() != Some(&input.step_ids[1])
        || stored.revision != state.expected_revision
    {
        return Err(invalid("構成工程が未予約か作品が編集済みです"));
    }
    // Validate against the actual document too; never overwrite user content.
    plan_build_script(
        candidate.history().project(),
        &input.recipe,
        &input.plan.script,
    )
    .map_err(invalid)?;
    let receipt = candidate.execute(request).map_err(edit_error)?;
    preflight(&SessionResponse::Edited {
        receipt: receipt.clone(),
        snapshot: sessions.snapshot(record.session_id.clone(), &candidate)?,
    })?;
    state.receipts.push(Receipt {
        id: receipt.operation_id.clone(),
        revision: receipt.revision,
    });
    state.expected_revision = receipt.revision;
    state.child_id = None;
    state.validate(&input)?;
    sessions
        .store
        .commit_job_application(
            stored.version,
            &StoredSession {
                id: record.session_id.clone(),
                revision: receipt.revision,
                version: stored
                    .version
                    .checked_add(1)
                    .ok_or_else(|| invalid("保存世代上限です"))?,
                snapshot: candidate.snapshot().map_err(edit_error)?.into(),
            },
            record,
            &serde_json::to_vec(&state).map_err(invalid)?,
        )
        .map_err(storage_error)?;
    sessions.events.publish(
        &record.session_id,
        receipt.revision,
        SessionEventKind::Edited,
        Some(receipt.operation_id.clone()),
    );
    Ok(receipt)
}
