//! Blocking I/O entry, run on a blocking worker. Reuse common document intents
//! and bindings; a parent receipt may be reconciled without rewriting the file.
use crate::{
    build_record::decode,
    build_state::{Receipt, SavedProject, invalid},
    session_save::document_error,
    sessions::{Sessions, storage_error},
};
use std::sync::{Arc, Mutex};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::{DocumentBinding, JobKind, StoredJob};

pub(crate) fn save(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
) -> Result<SavedProject, ApiError> {
    let (input, mut state, binding, plan) = {
        let mut sessions = shared.lock().map_err(invalid)?;
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
        let proof = state
            .preflight
            .as_ref()
            .ok_or_else(|| invalid("出力前検査がありません"))?;
        if current.history().project().id != input.plan.project_id
            || stored.revision != state.expected_revision
            || stored.revision != proof.revision
            || sungeki_storage::project_hash(current.history().project()).map_err(document_error)?
                != proof.content_hash
        {
            return Err(invalid("検査した作品から変更されています"));
        }
        let index = usize::from(input.speech_count) + 5;
        if state.saved.is_none()
            && (state.phase != JobPhase::Running
                || state.receipts.len() != index
                || state.child_id.as_ref() != Some(&input.step_ids[index]))
        {
            return Err(invalid("保存工程が未予約です"));
        }
        let binding = sessions
            .store
            .document_binding(&expected.session_id)
            .map_err(storage_error)?;
        if let Some(b) = &binding {
            if std::path::Path::new(&b.path) != input.document_path
                || b.saved_content_hash != proof.content_hash
            {
                return Err(invalid("別の保存先または内容が登録されています"));
            }
        }
        if state.saved.is_some() && binding.is_none() {
            return Err(invalid("保存済み文書の対応記録がありません"));
        }
        let plan = if binding.is_none() {
            let plan = sessions.prepare_save(
                expected.session_id.clone(),
                Some(input.document_path.to_string_lossy().into()),
                stored.revision,
            )?;
            if plan.path != input.document_path {
                sessions.release_save(&plan);
                sessions
                    .store
                    .abandon_save(&expected.session_id)
                    .map_err(storage_error)?;
                return Err(invalid("固定保存先の解決結果が変わりました"));
            }
            Some(plan)
        } else {
            None
        };
        (input, state, binding, plan)
    };
    let binding = if let Some(plan) = plan {
        // A new parent never overwrites an existing file, even an identical one.
        let outcome =
            sungeki_storage::save_document(&plan.path, &plan.project, None).map_err(document_error);
        let mut sessions = shared.lock().map_err(invalid)?;
        sessions.release_save(&plan);
        let stamp = match outcome {
            Ok(stamp) => stamp,
            Err(error) => {
                if error.code != sungeki_protocol::ErrorCode::IoError {
                    sessions
                        .store
                        .abandon_save(&expected.session_id)
                        .map_err(storage_error)?;
                }
                return Err(error);
            }
        };
        let binding = DocumentBinding {
            session_id: expected.session_id.clone(),
            path: plan.path.to_string_lossy().into(),
            stamp,
            saved_content_hash: plan.content_hash,
        };
        sessions.finish_save(binding.clone())?;
        binding
    } else {
        binding.unwrap()
    };
    // Reconcile a successful write whose parent receipt was not persisted.
    let loaded = sungeki_storage::load_document(&input.document_path).map_err(document_error)?;
    if loaded.stamp != binding.stamp
        || sungeki_storage::project_hash(&loaded.project).map_err(document_error)?
            != binding.saved_content_hash
    {
        return Err(invalid("保存した作品が外部で変更されています"));
    }
    let saved = SavedProject {
        revision: state.expected_revision,
        content_hash: binding.saved_content_hash,
        stamp: binding.stamp,
    };
    if let Some(prior) = &state.saved {
        if prior.stamp != saved.stamp
            || prior.content_hash != saved.content_hash
            || prior.revision != saved.revision
        {
            return Err(invalid("保存結果の記録が一致しません"));
        }
        return Ok(saved);
    }
    let mut sessions = shared.lock().map_err(invalid)?;
    if sessions
        .store
        .load_job(&expected.id)
        .map_err(storage_error)?
        .as_ref()
        != Some(expected)
        || sessions.load(&expected.session_id)?.0.revision != state.expected_revision
    {
        return Err(invalid(
            "保存中に制作状態が変更されました。文書は保存済みです",
        ));
    }
    state.receipts.push(Receipt {
        id: input.step_ids[state.receipts.len()].clone(),
        revision: saved.revision,
    });
    state.child_id = None;
    state.saved = Some(saved.clone());
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
    Ok(saved)
}
