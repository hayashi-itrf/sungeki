//! Read-only proof for explicit continuation. This never dispatches a child or
//! edits a project. A future continuation transaction must recheck these source
//! records and settlement again immediately before starting a new parent.
use crate::{
    build_record::decode,
    build_state::invalid,
    sessions::{Sessions, storage_error},
    speech_journal::Input,
    speech_state::{Application, Artifact},
};
use std::sync::{Arc, Mutex};
use sungeki_media::VerifiedWav;
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_storage::StoredJob;

pub(crate) struct VerifiedReuse {
    pub parent: StoredJob,
    pub child: StoredJob,
    pub target_input_hash: String,
    pub artifact: Artifact,
    // Keep the checked bytes, not just a mutable filesystem path.
    pub audio: VerifiedWav,
}

pub(super) fn same_audio(source: &Input, target: &Input) -> Result<bool, ApiError> {
    if std::mem::discriminant(source) != std::mem::discriminant(target)
        || source.lip_sync_settings() != target.lip_sync_settings()
    {
        return Ok(false);
    }
    let old = source.application(source.project())?;
    let new = target.application(target.project())?;
    Ok(old.fingerprint == new.fingerprint
        && old.engine_version == new.engine_version
        && serde_json::to_value((&old.irodori_source, &old.qwen_source, &old.sbv2_source))
            .map_err(invalid)?
            == serde_json::to_value((&new.irodori_source, &new.qwen_source, &new.sbv2_source))
                .map_err(invalid)?)
}

pub(crate) fn verify_blocking(
    shared: &Arc<Mutex<Sessions>>,
    parent_id: &str,
    child_id: &str,
    target: &Input,
) -> Result<VerifiedReuse, ApiError> {
    let (parent, child, artifact) = {
        let sessions = shared.lock().map_err(invalid)?;
        let parent = sessions
            .store
            .load_job(parent_id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("再利用元の制作がありません"))?;
        let child = sessions
            .store
            .load_job(child_id)
            .map_err(storage_error)?
            .ok_or_else(|| invalid("再利用元の音声がありません"))?;
        let (plan, state) = decode(&parent)?;
        let (speech, output) = crate::speech_journal::decode_input(&child)?;
        let index = plan
            .step_ids
            .iter()
            .position(|id| id == child_id)
            .filter(|index| *index >= 2 && *index < usize::from(plan.speech_count) + 2)
            .ok_or_else(|| invalid("再利用元の合成工程ではありません"))?;
        if !state.phase.is_terminal()
            || output.phase != JobPhase::Succeeded
            || parent.session_id != child.session_id
            || speech.parent_job_id() != Some(parent_id)
            || speech.clip_id() != plan.plan.script.speech_ids[index - 2]
            || !super::settlement::ready(&sessions, &parent)?
            || sessions.speech_runtime.owns(child_id)?
            || sessions
                .speech_resources
                .has_owner(child_id)
                .map_err(invalid)?
        {
            return Err(invalid("再利用元の音声の所有・終了を確認できません"));
        }
        let Application::Applied { revision, .. } = output.application else {
            return Err(invalid("再利用元の音声は制作に適用されていません"));
        };
        let (_, mut session) = sessions.load(&parent.session_id)?;
        let request = session
            .recorded_request(&plan.plan.speech_operation_ids[index - 2])
            .cloned()
            .ok_or_else(|| invalid("再利用元の適用記録がありません"))?;
        if state
            .receipts
            .get(index)
            .is_none_or(|r| r.id != child_id || r.revision != revision)
            || request.expected_revision != speech.source_revision()
            || session.execute(request).map_err(invalid)?.revision != revision
        {
            return Err(invalid("再利用元の親子と編集の記録が一致しません"));
        }
        if !same_audio(&speech, target)? {
            return Err(invalid(
                "発音入力・音声設定・エンジンが変わった音声は再利用できません",
            ));
        }
        let artifact = output
            .artifact
            .ok_or_else(|| invalid("再利用元の原本がありません"))?;
        (parent, child, artifact)
    };
    let audio = crate::speech_original::load(&artifact)?;
    let sessions = shared.lock().map_err(invalid)?;
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
        return Err(invalid("原本確認中に再利用元の記録が更新されました"));
    }
    if !super::settlement::ready(&sessions, &parent)? {
        return Err(invalid("原本確認中に元の制作の終了条件が変わりました"));
    }
    Ok(VerifiedReuse {
        parent,
        child,
        target_input_hash: target.input_hash().into(),
        artifact,
        audio,
    })
}

pub(crate) async fn verify(
    shared: &Arc<Mutex<Sessions>>,
    parent_id: &str,
    child_id: &str,
    target: &Input,
) -> Result<VerifiedReuse, ApiError> {
    let shared = shared.clone();
    let parent_id = parent_id.to_owned();
    let child_id = child_id.to_owned();
    let target = target.clone();
    tokio::task::spawn_blocking(move || verify_blocking(&shared, &parent_id, &child_id, &target))
        .await
        .map_err(invalid)?
}
