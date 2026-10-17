#[path = "build_speech_start.rs"]
pub(crate) mod managed;
#[path = "build_preparation.rs"]
pub(crate) mod preparation;
// Validate the durable parent reservation at child creation AND immediately
// before remote dispatch, including after asynchronous directory preparation.
use crate::{
    build_record::decode,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use sungeki_jobs::{BuildSchedule, BuildStep};
use sungeki_project::JobPhase;
use sungeki_protocol::ApiError;
use sungeki_speech::SpeechJobInput;

pub(crate) fn validate_parent(
    sessions: &Sessions,
    input: &crate::speech_journal::Input,
    job_id: &str,
    session_id: &str,
) -> Result<(), ApiError> {
    validate_parent_mode(sessions, input, job_id, session_id, false)
}

pub(super) fn validate_parent_mode(
    sessions: &Sessions,
    input: &crate::speech_journal::Input,
    job_id: &str,
    session_id: &str,
    recover_reused: bool,
) -> Result<(), ApiError> {
    let Some(parent_id) = input.parent_job_id() else {
        return Ok(());
    };
    let parent = sessions
        .store
        .load_job(parent_id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("制作親がありません"))?;
    let (plan, state) = decode(&parent)?;
    let schedule = BuildSchedule::restore(
        plan.speech_count,
        state.receipts.len() as u16,
        state.child_id.clone(),
    )
    .map_err(invalid)?;
    let Some(BuildStep::Speech(index)) = schedule.step() else {
        return Err(invalid("合成工程ではありません"));
    };
    let (stored, current) = sessions.load(session_id)?;
    if recover_reused {
        super::reuse_reconcile::validate(sessions, &parent, job_id)?;
    }
    if state.phase
        != if recover_reused {
            JobPhase::Interrupted
        } else {
            JobPhase::Running
        }
        || parent.session_id != session_id
        || state.child_id.as_deref() != Some(job_id)
        || plan.plan.script.speech_ids[usize::from(index)] != input.clip_id()
        || state.expected_revision != input.source_revision()
        || stored.revision != input.source_revision()
        || current.history().project() != input.project()
        || input.project().id != plan.plan.project_id
    {
        return Err(invalid("親の予約・対象・版が合成要求と一致しません"));
    }
    use crate::{build_record::profiles::Kind, speech_journal::Input};
    let managed = match input {
        Input::Voicevox(i) => {
            if plan
                .engine
                .as_ref()
                .is_some_and(|e| e.endpoint != i.endpoint() || &e.identity != i.identity())
            {
                return Err(invalid("親の合成先と子入力が一致しません"));
            }
            None
        }
        Input::Sbv2(i) => Some((
            Kind::Sbv2,
            i.profile_id(),
            i.prepared().map_err(invalid)?.character().id.clone(),
        )),
        Input::Qwen(i) => Some((
            Kind::Qwen,
            i.profile_id(),
            i.prepared().map_err(invalid)?.character().id.clone(),
        )),
        Input::Irodori(i) => Some((
            Kind::Irodori,
            i.profile_id(),
            i.prepared().map_err(invalid)?.character().id.clone(),
        )),
    };
    if let Some((kind, profile_id, character_id)) = managed {
        let fixed = plan
            .managed_profiles
            .get(&character_id)
            .ok_or_else(|| invalid("制作話者の固定設定がありません"))?;
        if fixed.kind != kind || profile_id != Some(fixed.profile_id.as_str()) {
            return Err(invalid("親の話者設定と子入力が一致しません"));
        }
    }
    Ok(())
}

/// Caller supplies an identity obtained from engine discovery. The existing
/// speech worker verifies it again under the engine resource lease.
pub(crate) fn start(
    shared: &std::sync::Arc<std::sync::Mutex<Sessions>>,
    expected: &sungeki_storage::StoredJob,
    endpoint: &str,
    identity: &sungeki_speech::VoicevoxIdentity,
) -> Result<sungeki_storage::StoredJob, ApiError> {
    let (child_id, input) = {
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
        let (plan, state) = decode(expected)?;
        let schedule = BuildSchedule::restore(
            plan.speech_count,
            state.receipts.len() as u16,
            state.child_id.clone(),
        )
        .map_err(invalid)?;
        let Some(BuildStep::Speech(index)) = schedule.step() else {
            return Err(invalid("合成工程ではありません"));
        };
        let child_id = state
            .child_id
            .ok_or_else(|| invalid("合成工程が未予約です"))?;
        let (_, current) = sessions.load(&expected.session_id)?;
        let input = SpeechJobInput::new(
            state.expected_revision,
            current.history().project(),
            &plan.plan.script.speech_ids[usize::from(index)],
            endpoint,
            identity,
        )
        .map_err(invalid)?
        .for_parent(&expected.id)
        .map_err(invalid)?;
        validate_parent(
            &sessions,
            &crate::speech_journal::Input::Voicevox(input.clone()),
            &child_id,
            &expected.session_id,
        )?;
        (child_id, input)
    };
    // start rechecks the parent after slow directory I/O via journal::create.
    // Existing child records are returned without dispatching them again.
    crate::speech_runtime::start(shared, expected.session_id.clone(), child_id, &input)
}
