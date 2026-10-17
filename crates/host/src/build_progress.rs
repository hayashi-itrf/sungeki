//! Read-only presentation of the durable cursor, never elapsed-time progress.
use crate::build_state::{Input, State, invalid};
use sungeki_jobs::{BuildSchedule, BuildStep};
use sungeki_project::{JobPhase, Progress};
use sungeki_protocol::ApiError;

pub(super) fn display(
    input: &Input,
    state: &State,
    sessions: &crate::sessions::Sessions,
    parent: &sungeki_storage::StoredJob,
) -> Result<Progress, ApiError> {
    // Cancellation timeout messages explain why completion is still unproven.
    if matches!(state.phase, JobPhase::Cancelling | JobPhase::Succeeded)
        && let Some(progress) = &state.progress
    {
        return Ok(progress.clone());
    }
    let stage = match state.phase {
        JobPhase::Queued => "制作開始待ち".into(),
        JobPhase::Cancelling => "取消中・処理の停止を確認しています".into(),
        JobPhase::Cancelled => "制作を取り消しました".into(),
        JobPhase::Interrupted => "制作が中断しました・記録を保持しています".into(),
        JobPhase::Failed => "制作に失敗しました".into(),
        JobPhase::Succeeded => "制作完了・確認レポートの未検査項目をご確認ください".into(),
        JobPhase::Running => {
            let schedule = BuildSchedule::restore(
                input.speech_count,
                state.receipts.len() as u16,
                state.child_id.clone(),
            )
            .map_err(invalid)?;
            match schedule
                .step()
                .ok_or_else(|| invalid("実行中の工程がありません"))?
            {
                BuildStep::Prepare => "素材とプリセットを確認中".into(),
                BuildStep::Compose => "台本を作品に組み込み中".into(),
                BuildStep::Speech(index) => {
                    format!(
                        "{}（{} / {}）",
                        speech_stage(
                            sessions,
                            parent,
                            state,
                            input
                                .continuation
                                .as_ref()
                                .is_some_and(|c| c.source_for(index).is_some())
                        )?,
                        index + 1,
                        input.speech_count
                    )
                }
                BuildStep::Layout => "立ち絵・字幕・BGMを配置中".into(),
                BuildStep::Validate => "動画出力前の検証中".into(),
                BuildStep::Bake => "事前描画の要否を確認中".into(),
                BuildStep::Save => "作品を保存中".into(),
                BuildStep::Render => render_stage(sessions, parent, state)?,
                BuildStep::Review => "確認レポートを作成中".into(),
            }
        }
    };
    Ok(Progress {
        stage,
        // Step costs vary radically; completed_steps / total_steps is not ETA.
        fraction: (state.phase == JobPhase::Succeeded).then_some(1.0),
    })
}

fn render_stage(
    sessions: &crate::sessions::Sessions,
    parent: &sungeki_storage::StoredJob,
    state: &State,
) -> Result<String, ApiError> {
    let Some(id) = &state.child_id else {
        return Ok("動画出力を準備中".into());
    };
    let Some(child) = sessions
        .store
        .load_job(id)
        .map_err(crate::sessions::storage_error)?
    else {
        return Ok("動画出力を準備中".into());
    };
    let (input, render) = crate::render_record::decode(&child)?;
    if child.session_id != parent.session_id
        || input.parent_job_id.as_deref() != Some(parent.id.as_str())
        || input.source_revision != state.expected_revision
        || state
            .saved
            .as_ref()
            .is_none_or(|saved| saved.content_hash != input.content_hash)
    {
        return Err(invalid("出力子の所有または保存内容が一致しません"));
    }
    Ok(match render.phase {
        JobPhase::Queued => "動画出力の順番待ち".into(),
        JobPhase::Running => render
            .progress
            .map(|p| p.stage)
            .unwrap_or_else(|| "映像・音声を出力して検証中".into()),
        JobPhase::Cancelling => "動画出力の停止を確認中".into(),
        // A finished child is not a completed parent: review and result binding remain.
        _ => "動画出力の結果を確認中".into(),
    })
}

fn speech_stage(
    sessions: &crate::sessions::Sessions,
    parent: &sungeki_storage::StoredJob,
    state: &State,
    reusing: bool,
) -> Result<&'static str, ApiError> {
    let preparing = if reusing {
        "保存済み音声を検査・引き継ぎ中"
    } else {
        "音声生成を準備中"
    };
    let Some(id) = &state.child_id else {
        return Ok(preparing);
    };
    let Some(child) = sessions
        .store
        .load_job(id)
        .map_err(crate::sessions::storage_error)?
    else {
        return Ok(preparing);
    };
    let (input, speech) = crate::speech_journal::decode_input(&child)?;
    if child.session_id != parent.session_id || input.parent_job_id() != Some(parent.id.as_str()) {
        return Err(invalid("子処理の所有が一致しません"));
    }
    if speech.execution == crate::speech_state::Execution::Reused {
        return Ok("引き継いだ音声を作品へ反映中");
    }
    Ok(match speech.phase {
        JobPhase::Queued => "音声生成の順番待ち",
        JobPhase::Running => match speech.progress.as_ref().map(|p| p.stage.as_str()) {
            Some("speech.inspect_originals" | "speech.recheck_originals") => {
                "音声モデル・原本を確認中"
            }
            Some("speech.start_engine") => "音声エンジンを起動中",
            Some("speech.synthesize") => "セリフを音声に変換中",
            Some("speech.verify_output") => "生成した音声を検証中",
            Some("speech.stop_engine") => "音声エンジンを停止中",
            Some("speech.save_audio") => "生成した音声を保存中",
            _ => "音声を生成中",
        },
        _ => "音声処理の結果を確認中",
    })
}
