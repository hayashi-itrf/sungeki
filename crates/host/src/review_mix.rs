//! Report-only measurement. Reuses the host's formal speech freshness gate and
//! the render crate's exact frozen mix; never edits or synthesizes audio.
use crate::review_record::Input;
use std::{fs, os::unix::fs::DirBuilderExt, sync::atomic::AtomicBool, time::Instant};
use sungeki_project::{CheckKind, ClipContent, Issue, Severity};
use sungeki_protocol::ReviewReport;

pub(super) fn inspect(
    input: &Input,
    started: Instant,
    flag: &AtomicBool,
    report: &mut ReviewReport,
) {
    let active_audio = input.project.clips.iter().any(|c| {
        matches!(
            c.content,
            ClipContent::Speech { .. } | ClipContent::Audio { .. }
        ) && input
            .project
            .tracks
            .iter()
            .any(|t| t.id == c.track_id && !t.muted)
    });
    if !active_audio {
        return;
    }
    let Some(visual) = &input.visual else {
        return;
    };
    let directory = visual.directory.join("audio-inspection");
    let plan = crate::audio_preview_api::plan(&input.project);
    let remaining = visual.runtime.deadline.saturating_sub(started.elapsed());
    let result = if remaining.is_zero() {
        Err("確認ジョブの実行期限に達したため音量は未測定です。".into())
    } else if let Err(error) = plan.as_ref() {
        Err(format!(
            "正式出力の音声条件を満たしていないため未測定です: {}",
            error.message
        ))
    } else if let Err(error) = fs::DirBuilder::new().mode(0o700).create(&directory) {
        Err(format!("音量検査の作業領域を作れません: {error}"))
    } else {
        let measured = sungeki_render::inspect_output_audio(
            &plan.unwrap(),
            &sungeki_render::RenderRuntime {
                deadline: remaining,
                ..visual.runtime.clone()
            },
            &directory,
            flag,
        )
        .map_err(|e| e.to_string());
        match fs::remove_dir_all(&directory) {
            Ok(()) => measured,
            Err(error) => Err(format!("音量検査の一時領域を片付けられません: {error}")),
        }
    };
    match result {
        Ok(analysis) => {
            let (description, warning) = sungeki_review::describe_mix_loudness(&analysis);
            if let Some(check) = report
                .diagnostics
                .checks
                .iter_mut()
                .find(|c| c.kind == CheckKind::Audio)
            {
                check.description = format!(
                    "登録尺による配置と原本の検査結果は指摘欄を参照してください。使用音声を正式出力と同じ復号・gain・BGM調整・全体音量設定で混合しました。{description} AAC圧縮後の音量・聴き取りは未検査です。"
                );
            }
            if let Some((code, message)) = warning {
                issue(report, code, message);
            }
        }
        Err(message) => issue(report, "AUDIO_MIX_NOT_INSPECTED", message),
    }
}
fn issue(report: &mut ReviewReport, code: &str, message: String) {
    report.diagnostics.issues.push(Issue {
        severity: Severity::Warning,
        code: code.into(),
        field: "/audio_mix".into(),
        clip_id: None,
        scene_id: None,
        asset_id: None,
        start_us: None,
        end_us: None,
        message,
    });
}
