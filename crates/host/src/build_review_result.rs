//! Completion means the production steps ran; report diagnostics retain not_run/failed.
use crate::{
    build_record::decode,
    build_state::{Output, Receipt, invalid},
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::ApiError;
use sungeki_storage::{JobKind, StoredJob};

/// Blocking original/output verification, outside the session lock.
pub(crate) fn accept(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<Output, ApiError> {
    let (input, mut state, child, capture, report) = {
        let sessions = shared.lock().map_err(invalid)?;
        unchanged(&sessions, expected)?;
        let (input, state) = decode(expected)?;
        let index = usize::from(input.speech_count) + 7;
        let child = crate::review_jobs::record(&sessions, &input.step_ids[index])?;
        let (capture, result) = crate::review_record::decode(&child)?;
        let report = result
            .report
            .ok_or_else(|| invalid("確認レポートがありません"))?;
        let saved = state
            .saved
            .as_ref()
            .ok_or_else(|| invalid("保存記録がありません"))?;
        if result.phase != JobPhase::Succeeded
            || result.error.is_some()
            || child.session_id != expected.session_id
            || capture.parent_job_id.as_deref() != Some(expected.id.as_str())
            || capture.source_revision != saved.revision
            || capture.content_hash != saved.content_hash
            || capture.project.id != input.plan.project_id
            || report.report_id != child.id
            || report.source_revision != saved.revision
            || report.content_hash != saved.content_hash
        {
            return Err(invalid("確認結果と制作親・保存作品が一致しません"));
        }
        if let Some(prior) = &state.output {
            if serde_json::to_value(&prior.review).map_err(invalid)?
                != serde_json::to_value(&report).map_err(invalid)?
            {
                return Err(invalid("受領済みの確認レポートが変更されました"));
            }
        } else {
            crate::build_record::validate_review_parent(
                &sessions,
                &expected.id,
                &child.id,
                &expected.session_id,
                saved.revision,
            )?;
        }
        (input, state, child, capture, report)
    };
    crate::build_record::accept_render(shared, expected)?;
    // The image review cannot fingerprint audio/PSD. Verify used source bytes
    // independently, without turning any report's uninspected item into passed.
    crate::render_api::plan(&capture.project)?
        .verify_original_hashes()
        .map_err(crate::render_record::render_error)?;
    let mut sessions = shared.lock().map_err(invalid)?;
    unchanged(&sessions, expected)?;
    if sessions
        .store
        .load_job(&child.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(&child)
    {
        return Err(invalid("確認中に子レポートが変更されました"));
    }
    let (stored, current) = sessions.load(&expected.session_id)?;
    if stored.revision != capture.source_revision
        || sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
            != capture.content_hash
    {
        return Err(invalid("確認中に作品が変更されました"));
    }
    let saved = state.saved.as_ref().unwrap();
    let binding = sessions
        .store
        .document_binding(&expected.session_id)
        .map_err(storage_error)?;
    if binding.as_ref().is_none_or(|b| {
        std::path::Path::new(&b.path) != input.document_path
            || b.stamp != saved.stamp
            || b.saved_content_hash != saved.content_hash
    }) {
        return Err(invalid("確認中に保存先が変更されました"));
    }
    if let Some(output) = state.output {
        return Ok(output);
    }
    let output = Output {
        project_path: input.document_path.clone(),
        project_sha256: state.saved.as_ref().unwrap().stamp.hash.clone(),
        render_job_id: input.step_ids[input.step_ids.len() - 2].clone(),
        review_job_id: child.id.clone(),
        review: report,
    };
    state.receipts.push(Receipt {
        id: child.id,
        revision: stored.revision,
    });
    state.child_id = None;
    state.phase = JobPhase::Succeeded;
    state.progress = Some(Progress {
        stage: "制作処理完了（確認レポートの指摘・未検査項目を確認してください）".into(),
        fraction: Some(1.0),
    });
    state.output = Some(output.clone());
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
    sessions.build_runtime.notify_terminal();
    Ok(output)
}
fn unchanged(sessions: &Sessions, expected: &StoredJob) -> Result<(), ApiError> {
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
    {
        return Err(invalid("親記録が更新済みです"));
    }
    Ok(())
}
