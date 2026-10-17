//! Read-only choices for an explicit new production. Originals are reverified by
//! Start's existing continuation path; a journal row alone does not certify media.
use crate::{
    build_record as build,
    build_state::invalid,
    sessions::{Sessions, storage_error},
};
use sungeki_project::JobPhase;
use sungeki_protocol::{ApiError, BuildReuseLine, BuildReuseSource};

pub(super) fn read(sessions: &Sessions, id: &str) -> Result<BuildReuseSource, ApiError> {
    let parent = sessions
        .store
        .load_job(id)
        .map_err(storage_error)?
        .ok_or_else(|| invalid("引き継ぎ元の制作がありません"))?;
    let (plan, state) = build::decode(&parent)?;
    if !state.phase.is_terminal() || !build::settlement::ready(sessions, &parent)? {
        return Err(invalid("引き継ぎ元の処理がまだ終了していません"));
    }
    let mut lines = Vec::new();
    for (index, line) in plan.recipe.scenes.iter().flat_map(|s| &s.lines).enumerate() {
        let role = plan
            .recipe
            .roles
            .iter()
            .find(|r| r.role == line.role)
            .ok_or_else(|| invalid("保存された台本の話者がありません"))?;
        let id = &plan.step_ids[index + 2];
        let mut has_saved_audio = false;
        if let Some(child) = sessions.store.load_job(id).map_err(storage_error)? {
            let (input, output) = crate::speech_journal::decode_input(&child)?;
            if child.session_id != parent.session_id
                || input.parent_job_id() != Some(parent.id.as_str())
                || input.clip_id() != plan.plan.script.speech_ids[index]
            {
                return Err(invalid("音声の所属が制作記録と一致しません"));
            }
            if let crate::speech_state::Application::Applied { revision, .. } = output.application {
                has_saved_audio = output.phase == JobPhase::Succeeded
                    && output.artifact.is_some()
                    && state
                        .receipts
                        .get(index + 2)
                        .is_some_and(|r| r.id == *id && r.revision == revision);
            }
        }
        lines.push(BuildReuseLine {
            index: index as u16,
            character_id: role.character.id.clone(),
            character_name: role.character.name.clone(),
            subtitle_text: line.subtitle_text.clone(),
            spoken_text: line.spoken_text.clone(),
            has_saved_audio,
        });
    }
    Ok(BuildReuseSource {
        base_url: plan
            .engine
            .as_ref()
            .map(|e| e.endpoint.clone())
            .unwrap_or_default(),
        engine_version: plan
            .engine
            .as_ref()
            .map(|e| e.identity.engine_version.clone())
            .unwrap_or_default(),
        job_id: parent.id,
        title: plan.recipe.title,
        lines,
    })
}
