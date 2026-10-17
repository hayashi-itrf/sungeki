//! Blocking byte verification after the renderer's codec/decode validation.
use crate::{
    build_record::decode,
    build_state::{Receipt, invalid},
    sessions::{Sessions, storage_error},
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, RenderOutput};
use sungeki_storage::{JobKind, StoredJob};

pub(crate) fn accept(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<RenderOutput, ApiError> {
    let (input, mut state, child, output) = {
        let sessions = shared.lock().map_err(invalid)?;
        unchanged(&sessions, expected)?;
        let (input, state) = decode(expected)?;
        let index = usize::from(input.speech_count) + 6;
        let child = crate::render_record::record(&sessions, &input.step_ids[index])?;
        let (capture, result) = crate::render_record::decode(&child)?;
        let saved = state
            .saved
            .as_ref()
            .ok_or_else(|| invalid("保存記録がありません"))?;
        let output = result
            .output
            .ok_or_else(|| invalid("動画出力が完了していません"))?;
        if capture.parent_job_id.as_deref() != Some(&expected.id)
            || child.session_id != expected.session_id
            || capture.source_revision != saved.revision
            || capture.content_hash != saved.content_hash
            || capture.project.id != input.plan.project_id
            || result.phase != JobPhase::Succeeded
            || result.error.is_some()
            || Path::new(&output.path) != capture.directory.join("output.mp4")
            || output.font_sha256 != capture.runtime.font_sha256
            || state
                .preflight
                .as_ref()
                .is_none_or(|p| p.frame_count != output.frame_count)
        {
            return Err(invalid("動画出力と保存作品・制作親が一致しません"));
        }
        if let Some(prior) = &state.rendered {
            if serde_json::to_value(prior).map_err(invalid)?
                != serde_json::to_value(&output).map_err(invalid)?
            {
                return Err(invalid("受領済み動画の記録が変わりました"));
            }
        } else {
            crate::build_record::validate_render_parent(
                &sessions,
                &expected.id,
                &child.id,
                &expected.session_id,
                saved.revision,
            )?;
        }
        (input, state, child, output)
    };
    // Already-saved path only: rechecks the document and binding without rewriting.
    crate::build_record::save_project(shared, expected)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    sungeki_storage::verify_output(
        Path::new(&output.path),
        &output.sha256,
        output.byte_length,
        || {
            deadline <= Instant::now()
                || shared
                    .lock()
                    .map_or(true, |sessions| unchanged(&sessions, expected).is_err())
        },
    )
    .map_err(invalid)?;
    let mut sessions = shared.lock().map_err(invalid)?;
    unchanged(&sessions, expected)?;
    if sessions
        .store
        .load_job(&child.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(&child)
    {
        return Err(invalid("確認中に動画出力の記録が変更されました"));
    }
    let (stored, current) = sessions.load(&expected.session_id)?;
    let saved = state.saved.as_ref().unwrap();
    if stored.revision != saved.revision
        || sungeki_storage::project_hash(current.history().project()).map_err(invalid)?
            != saved.content_hash
    {
        return Err(invalid("確認中に作品が変更されました"));
    }
    let binding = sessions
        .store
        .document_binding(&expected.session_id)
        .map_err(storage_error)?;
    if binding.as_ref().is_none_or(|b| {
        Path::new(&b.path) != input.document_path
            || b.stamp != saved.stamp
            || b.saved_content_hash != saved.content_hash
    }) {
        return Err(invalid("確認中に作品の保存先が変更されました"));
    }
    if state.rendered.is_none() {
        state.rendered = Some(output.clone());
        state.receipts.push(Receipt {
            id: child.id,
            revision: saved.revision,
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
    }
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
