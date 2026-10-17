//! Blocking bridge from a reserved build step to the shared speech scheduler.
use crate::{
    build_record::{
        self as build,
        profiles::{FrozenProfile, LoadedProfile},
    },
    build_state::invalid,
    sessions::{Sessions, storage_error},
    speech_journal::Input,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sungeki_project::{ClipContent, Project, Voice};
use sungeki_protocol::ApiError;
use sungeki_storage::StoredJob;

pub(crate) fn profile_for_clip(
    plan: &crate::build_state::Input,
    project: &Project,
    clip_id: &str,
) -> Result<Option<FrozenProfile>, ApiError> {
    let clip = project
        .clips
        .iter()
        .find(|c| c.id == clip_id)
        .ok_or_else(|| invalid("制作の音声クリップがありません"))?;
    let ClipContent::Speech { character_id, .. } = &clip.content else {
        return Err(invalid("制作対象が音声ではありません"));
    };
    let character = project
        .characters
        .iter()
        .find(|c| &c.id == character_id)
        .ok_or_else(|| invalid("制作話者がありません"))?;
    if matches!(character.voice, Voice::Voicevox { .. }) {
        return Ok(None);
    }
    plan.managed_profiles
        .get(character_id)
        .cloned()
        .map(Some)
        .ok_or_else(|| invalid("制作話者の固定設定がありません"))
}

pub(crate) fn start(
    shared: &Arc<Mutex<Sessions>>,
    expected: &StoredJob,
    connection: Option<build::SpeechConnection>,
    flag: &AtomicBool,
) -> Result<StoredJob, ApiError> {
    if flag.load(Ordering::Acquire) {
        return Err(ApiError::new(
            sungeki_protocol::ErrorCode::Cancelled,
            "制作音声の準備を中止しました",
        ));
    }
    let (database, project, revision, clip_id, child_id, fixed) = {
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
        let (plan, state) = build::decode(expected)?;
        let schedule = sungeki_jobs::BuildSchedule::restore(
            plan.speech_count,
            state.receipts.len() as u16,
            state.child_id.clone(),
        )
        .map_err(invalid)?;
        let Some(sungeki_jobs::BuildStep::Speech(index)) = schedule.step() else {
            return Err(invalid("合成工程ではありません"));
        };
        let (_, current) = sessions.load(&expected.session_id)?;
        let project = current.history().project().clone();
        let clip = plan.plan.script.speech_ids[usize::from(index)].clone();
        let fixed = profile_for_clip(&plan, &project, &clip)?;
        (
            sessions.review_database_path.clone(),
            project,
            state.expected_revision,
            clip,
            state
                .child_id
                .ok_or_else(|| invalid("音声工程が未予約です"))?,
            fixed,
        )
    };
    let Some(fixed) = fixed else {
        let connection = connection.ok_or_else(|| invalid("VOICEVOX接続がありません"))?;
        return build::start_speech(shared, expected, &connection.endpoint, &connection.identity);
    };
    let profile = LoadedProfile::load(&database, fixed.kind, &fixed.profile_id, flag)?;
    let input = profile.prepare_child(&fixed, &expected.id, revision, &project, &clip_id, flag)?;
    {
        let sessions = shared.lock().map_err(invalid)?;
        if sessions
            .store
            .load_job(&expected.id)
            .map_err(storage_error)?
            .as_ref()
            != Some(expected)
        {
            return Err(invalid("原本確認中に制作状態が変わりました"));
        }
        if flag.load(Ordering::Acquire) {
            return Err(ApiError::new(
                sungeki_protocol::ErrorCode::Cancelled,
                "制作音声の準備を中止しました",
            ));
        }
        input.validate_parent(&sessions, &child_id, &expected.session_id)?;
    }
    // Runtime repeats parent validation after directory preparation and owns
    // resource leases, process shutdown, and durable audio application.
    let sid = expected.session_id.clone();
    match (profile, input) {
        (LoadedProfile::Sbv2(p), Input::Sbv2(i)) => {
            crate::speech_runtime::start_sbv2(shared, sid, child_id, &i, p.binding, p.cpu_only)
        }
        (LoadedProfile::Qwen(p), Input::Qwen(i)) => {
            crate::speech_runtime::start_qwen(shared, sid, child_id, &i, p.binding, p.port)
        }
        (LoadedProfile::Irodori(p), Input::Irodori(i)) => {
            crate::speech_runtime::start_irodori(shared, sid, child_id, &i, p.binding, p.port)
        }
        _ => Err(invalid("準備した音声と起動設定のエンジンが一致しません")),
    }
}
