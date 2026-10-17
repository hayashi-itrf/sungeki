//! Persist the final visual layout and parent receipt as one editor transaction.
use crate::{
    build_record::decode,
    build_state::invalid,
    sessions::{Sessions, edit_error, preflight, storage_error},
};
use sungeki_editor::{EditAction, EditReceipt, EditRequest, plan_build_layout};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, SessionEventKind, SessionResponse};
use sungeki_storage::{StoredJob, StoredSession};

pub(crate) fn layout(sessions: &mut Sessions, record: &StoredJob) -> Result<EditReceipt, ApiError> {
    if sessions
        .store
        .load_job(&record.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(record)
    {
        return Err(invalid("親記録が更新済みです"));
    }
    let (input, mut state) = decode(record)?;
    let index = usize::from(input.speech_count) + 2;
    let operation_id = &input.step_ids[index];
    let (stored, mut candidate) = sessions.load(&record.session_id)?;
    if candidate.history().project().id != input.plan.project_id {
        return Err(invalid("制作先が一致しません"));
    }
    if let Some(completed) = state.receipts.get(index) {
        let request = candidate
            .recorded_request(operation_id)
            .cloned()
            .ok_or_else(|| invalid("映像構成の編集記録がありません"))?;
        if request.expected_revision != state.receipts[index - 1].revision
            || !matches!(request.action, EditAction::Apply { .. })
        {
            return Err(invalid("映像構成の編集要求が一致しません"));
        }
        let receipt = candidate.execute(request).map_err(edit_error)?;
        if receipt.revision != completed.revision {
            return Err(invalid("映像構成のrevisionが一致しません"));
        }
        return Ok(receipt);
    }
    if state.receipts.len() != index
        || state.phase != JobPhase::Running
        || state.child_id.as_ref() != Some(operation_id)
        || stored.revision != state.expected_revision
    {
        return Err(invalid("映像構成工程が未予約か作品が編集済みです"));
    }
    for n in 0..usize::from(input.speech_count) {
        let id = &input.step_ids[n + 2];
        let child = sessions
            .store
            .load_job(id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("合成子がありません"))?;
        let (speech, child_state) = crate::speech_journal::decode_input(&child)?;
        if child.session_id != record.session_id
            || speech.parent_job_id() != Some(record.id.as_str())
            || speech.clip_id() != input.plan.script.speech_ids[n]
            || child_state.phase != JobPhase::Succeeded
            || speech.source_revision() != state.receipts[n + 1].revision
            || !matches!(child_state.application,crate::speech_state::Application::Applied {revision,..} if revision==state.receipts[n+2].revision)
        {
            return Err(invalid("合成子の適用記録が親と一致しません"));
        }
    }
    let commands = plan_build_layout(
        candidate.history().project(),
        &input.recipe,
        &input.plan.script,
        &input.plan.layout,
    )
    .map_err(invalid)?;
    let receipt = candidate
        .execute(EditRequest {
            operation_id: operation_id.clone(),
            expected_revision: stored.revision,
            action: EditAction::Apply { commands },
        })
        .map_err(edit_error)?;
    preflight(&SessionResponse::Edited {
        receipt: receipt.clone(),
        snapshot: sessions.snapshot(record.session_id.clone(), &candidate)?,
    })?;
    state.receipts.push(crate::build_state::Receipt {
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
                id: stored.id,
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
