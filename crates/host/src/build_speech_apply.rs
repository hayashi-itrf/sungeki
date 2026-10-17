//! Revalidate a parent's saved speech original, then commit all three records.
use crate::{
    build_record::decode,
    build_state::{Receipt, invalid},
    sessions::{Sessions, edit_error, preflight, storage_error},
    speech_state::Application,
};
use std::sync::{Arc, Mutex};
use sungeki_editor::{EditAction, EditReceipt, EditRequest, plan_build_speech_application};
use sungeki_project::{Asset, AssetLocator, AssetProbe, JobPhase, SpeechAudio};
use sungeki_protocol::{ApiError, SessionEventKind, SessionResponse};

pub(crate) async fn apply(
    shared: &Arc<Mutex<Sessions>>,
    parent_id: &str,
    child_id: &str,
) -> Result<EditReceipt, ApiError> {
    apply_mode(shared, parent_id, child_id, false).await
}

pub(super) async fn apply_mode(
    shared: &Arc<Mutex<Sessions>>,
    parent_id: &str,
    child_id: &str,
    recover_reused: bool,
) -> Result<EditReceipt, ApiError> {
    let (parent, child, input, mut state, speech, mut child_state, index, artifact) = {
        let sessions = shared.lock().map_err(invalid)?;
        let parent = sessions
            .store
            .load_job(parent_id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("親がありません"))?;
        let child = sessions
            .store
            .load_job(child_id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("合成子がありません"))?;
        let (input, state) = decode(&parent)?;
        let (speech, child_state) = crate::speech_journal::decode_input(&child)?;
        let index = input
            .step_ids
            .iter()
            .position(|id| id == child_id)
            .filter(|i| *i >= 2 && *i < usize::from(input.speech_count) + 2)
            .ok_or_else(|| invalid("親の合成工程ではありません"))?;
        if child.session_id != parent.session_id
            || speech.parent_job_id() != Some(parent_id)
            || speech.clip_id() != input.plan.script.speech_ids[index - 2]
            || child_state.phase != JobPhase::Succeeded
        {
            return Err(invalid("成功した親所属の音声ではありません"));
        }
        if let Application::Applied { revision, .. } = child_state.application {
            let (_, mut session) = sessions.load(&parent.session_id)?;
            let recorded = session
                .recorded_request(&input.plan.speech_operation_ids[index - 2])
                .cloned()
                .ok_or_else(|| invalid("音声編集の記録がありません"))?;
            if state
                .receipts
                .get(index)
                .is_none_or(|r| r.id != child_id || r.revision != revision)
                || recorded.expected_revision != speech.source_revision()
            {
                return Err(invalid("親子の適用記録が一致しません"));
            }
            let receipt = session.execute(recorded).map_err(edit_error)?;
            if receipt.revision != revision {
                return Err(invalid("適用revisionが一致しません"));
            }
            return Ok(receipt);
        }
        if recover_reused {
            super::reuse_reconcile::validate(&sessions, &parent, child_id)?;
            if child_state.execution != crate::speech_state::Execution::Reused {
                return Err(invalid("復旧対象は引き継いだ音声に限定されます"));
            }
        }
        if !matches!(child_state.application, Application::PendingConflict)
            || state.phase
                != if recover_reused {
                    JobPhase::Interrupted
                } else {
                    JobPhase::Running
                }
            || state.receipts.len() != index
            || state.child_id.as_deref() != Some(child_id)
            || state.expected_revision != speech.source_revision()
        {
            return Err(invalid("反映可能な親工程ではありません"));
        }
        let artifact = child_state
            .artifact
            .clone()
            .ok_or_else(|| invalid("音声原本がありません"))?;
        (
            parent,
            child,
            input,
            state,
            speech,
            child_state,
            index,
            artifact,
        )
    };
    // Large I/O stays outside the session lock. Recheck both records afterwards.
    let checked = artifact.clone();
    let _verified = tokio::task::spawn_blocking(move || crate::speech_original::load(&checked))
        .await
        .map_err(invalid)??;
    let mut sessions = shared.lock().map_err(invalid)?;
    if sessions
        .store
        .load_job(parent_id)
        .map_err(storage_error)?
        .as_ref()
        != Some(&parent)
        || sessions
            .store
            .load_job(child_id)
            .map_err(storage_error)?
            .as_ref()
            != Some(&child)
    {
        return Err(invalid("原本確認中に親または子が更新されました"));
    }
    if recover_reused {
        super::reuse_reconcile::validate(&sessions, &parent, child_id)?;
    }
    let (stored, mut candidate) = sessions.load(&parent.session_id)?;
    let prepared = speech.application(candidate.history().project())?;
    if stored.revision != state.expected_revision
        || candidate.history().project() != speech.project()
        || candidate.history().project().id != input.plan.project_id
        || !prepared.matches
    {
        return Err(invalid("合成後に制作対象が変更されました"));
    }
    let asset_id = input.plan.speech_asset_ids[index - 2].clone();
    let commands = plan_build_speech_application(
        candidate.history().project(),
        &input.recipe,
        &input.plan.script,
        speech.clip_id(),
        Asset {
            id: asset_id.clone(),
            name: "合成音声".into(),
            source: Some(format!("speech-job:{child_id}")),
            credit: prepared.credit,
            locator: AssetLocator::Absolute {
                path: artifact.path.clone(),
            },
            content_hash: Some(artifact.sha256.clone()),
            probe: AssetProbe::Audio(artifact.probe()),
        },
        SpeechAudio {
            irodori_source: prepared.irodori_source,
            qwen_source: prepared.qwen_source,
            sbv2_source: prepared.sbv2_source,
            asset_id,
            input_fingerprint: prepared.fingerprint,
            engine_version: prepared.engine_version,
            lip_sync: artifact.lip_sync.clone(),
        },
    )
    .map_err(invalid)?;
    let receipt = candidate
        .execute(EditRequest {
            operation_id: input.plan.speech_operation_ids[index - 2].clone(),
            expected_revision: stored.revision,
            action: EditAction::Apply { commands },
        })
        .map_err(edit_error)?;
    preflight(&SessionResponse::Edited {
        receipt: receipt.clone(),
        snapshot: sessions.snapshot(parent.session_id.clone(), &candidate)?,
    })?;
    child_state.application = Application::Applied {
        revision: receipt.revision,
        overlapping_clip_ids: vec![],
    };
    crate::speech_transition::validate(
        &crate::speech_journal::decode_input(&child)?.1,
        &child_state,
    )?;
    state.receipts.push(Receipt {
        id: child_id.into(),
        revision: receipt.revision,
    });
    state.expected_revision = receipt.revision;
    state.child_id = None;
    state.validate(&input)?;
    sessions
        .store
        .commit_build_speech_application(
            stored.version,
            &sungeki_storage::StoredSession {
                id: stored.id,
                revision: receipt.revision,
                version: stored
                    .version
                    .checked_add(1)
                    .ok_or_else(|| invalid("保存世代上限です"))?,
                snapshot: candidate.snapshot().map_err(edit_error)?.into(),
            },
            &parent,
            &serde_json::to_vec(&state).map_err(invalid)?,
            &child,
            &crate::speech_journal::encode(&child_state)?,
        )
        .map_err(storage_error)?;
    sessions.events.publish(
        &parent.session_id,
        receipt.revision,
        SessionEventKind::Edited,
        Some(receipt.operation_id.clone()),
    );
    Ok(receipt)
}
