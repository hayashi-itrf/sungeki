//! Apply a verified original through the shared editor and one durable DB transaction.
use crate::{
    sessions::{Sessions, edit_error, preflight, storage_error},
    speech_journal::decode_input as decode,
    speech_state::{Application, State, invalid},
};

use sungeki_editor::{Command, EditAction, EditRequest, plan_speech_timing};
use sungeki_project::{Asset, AssetLocator, AssetProbe, JobPhase, SpeechAudio, TimeUs};
use sungeki_protocol::{ApiError, SessionEventKind, SessionResponse};
use sungeki_storage::{StoredJob, StoredSession};

/// Called under the session lock after storing/revalidating the WAV outside it.
pub(crate) fn finish(
    sessions: &mut Sessions,
    record: &StoredJob,
    mut state: State,
) -> Result<(), ApiError> {
    let (input, old) = decode(record)?;
    crate::speech_journal::validate_input_timing(&input, &state)?;
    if state.phase != JobPhase::Succeeded {
        return Err(invalid("未成功の音声を反映できません"));
    }
    let artifact = state
        .artifact
        .clone()
        .ok_or_else(|| invalid("原本がありません"))?;
    // Parent-owned synthesis only stores the verified original. The parent
    // applies audio plus ripple timing atomically; never run both edit paths.
    if input.parent_job_id().is_some() {
        return crate::speech_journal::save(sessions, record, &state);
    }
    let (stored, mut candidate) = sessions.load(&record.session_id)?;
    let current = candidate.history().project();
    let prepared = input.application(current)?;
    if !prepared.matches {
        return crate::speech_journal::save(sessions, record, &state);
    }
    let plan = match plan_speech_timing(current, input.clip_id(), TimeUs(artifact.duration_us)) {
        Ok(plan) => plan,
        Err(_) => return crate::speech_journal::save(sessions, record, &state), // Keep original and pending result for explicit resolution.
    };
    let asset_id = uuid::Uuid::new_v4().to_string();
    let mut commands = plan.commands;
    commands.push(Command::RegisterAsset {
        asset: Asset {
            id: asset_id.clone(),
            name: "合成音声".into(),
            source: Some(format!("speech-job:{}", record.id)),
            credit: prepared.credit,
            locator: AssetLocator::Absolute {
                path: artifact.path.clone(),
            },
            content_hash: Some(artifact.sha256.clone()),
            probe: AssetProbe::Audio(artifact.probe()),
        },
    });
    commands.push(Command::SetSpeechAudio {
        clip_id: input.clip_id().into(),
        audio: Some(SpeechAudio {
            irodori_source: prepared.irodori_source,
            qwen_source: prepared.qwen_source,
            sbv2_source: prepared.sbv2_source,
            asset_id,
            input_fingerprint: prepared.fingerprint,
            engine_version: prepared.engine_version,
            lip_sync: artifact.lip_sync.clone(),
        }),
    });
    let receipt = candidate
        .execute(EditRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: stored.revision,
            action: EditAction::Apply { commands },
        })
        .map_err(edit_error)?;
    preflight(&SessionResponse::Edited {
        receipt: receipt.clone(),
        snapshot: sessions.snapshot(record.session_id.clone(), &candidate)?,
    })?;
    state.application = Application::Applied {
        revision: receipt.revision,
        overlapping_clip_ids: plan.overlapping_clip_ids,
    };
    state.validate()?;
    crate::speech_transition::validate(&old, &state)?;
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
                    .ok_or_else(|| invalid("保存世代の上限です"))?,
                snapshot: candidate.snapshot().map_err(edit_error)?.into(),
            },
            &record,
            &crate::speech_journal::encode(&state)?,
        )
        .map_err(storage_error)?;
    sessions.speech_runtime.notify_changed();
    sessions.events.publish(
        &record.session_id,
        receipt.revision,
        SessionEventKind::Edited,
        Some(receipt.operation_id),
    );
    Ok(())
}
