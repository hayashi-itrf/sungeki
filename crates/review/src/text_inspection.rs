use crate::{Check, CheckKind, CheckStatus, FrameSample, Issue, Severity};
use std::{fs, os::unix::fs::DirBuilderExt, path::Path, sync::atomic::AtomicBool};
use sungeki_project::Project;
use sungeki_render::{RenderError, RenderRuntime};

pub(crate) fn inspect(
    project: &Project,
    margin_percent: u8,
    runtime: &RenderRuntime,
    root: &Path,
    cancelled: &AtomicBool,
    issues: &mut Vec<Issue>,
) -> Check {
    let mut check = Check {
        kind: CheckKind::TextLayout,
        status: CheckStatus::NotRun,
        description:
            "文字の全区間検査を実施できませんでした。安全余白・重なり・読みやすさは未検査です。"
                .into(),
    };
    let directory = root.join("text-inspection");
    // Only a directory successfully created by this call may be cleaned.
    if let Err(e) = fs::DirBuilder::new().mode(0o700).create(&directory) {
        unavailable(issues, format!("文字検査の作業領域を作れません: {e}"));
        return check;
    }
    let result = sungeki_render::inspect_text_layout(
        project,
        margin_percent,
        runtime,
        &directory,
        cancelled,
    );
    match result {
        Ok(result) => {
            let attempted = result.observations.len();
            if result.omitted_details > 0 {
                if !issues.iter().any(|i| i.code == "TEXT_DIAGNOSTICS_LIMIT") {
                    issues.push(Issue {
                        severity: Severity::Warning,
                        code: "TEXT_DIAGNOSTICS_LIMIT".into(),
                        field: "/clips".into(),
                        clip_id: None,
                        scene_id: None,
                        asset_id: None,
                        start_us: None,
                        end_us: None,
                        message: format!(
                            "文字の指摘の詳細{}件を上限により省略しました。",
                            result.omitted_details
                        ),
                    });
                }
            }
            let mut inspected = 0;
            let mut overflow = 0;
            let mut margin_warnings = 0;
            for observation in result.observations {
                if !observation.margin_issues.is_empty() {
                    margin_warnings += 1;
                    crate::text_margin::append(project, &observation, margin_percent, issues);
                }
                match observation.error {
                    None => inspected += 1,
                    Some(error @ RenderError::TextLayout(_)) => {
                        inspected += 1;
                        overflow += 1;
                        let time = project
                            .canvas
                            .fps
                            .frame_time(observation.frame_index)
                            .unwrap();
                        // Replace narrower representative-frame findings for this interval.
                        issues.retain(|issue| {
                            !(matches!(&error, RenderError::TextLayout(details) if !details.is_empty())
                                && issue.code == "TEXT_OVERFLOW"
                                && issue.clip_id.as_deref() == Some(observation.clip_id.as_str())
                                && issue
                                    .start_us
                                    .is_some_and(|t| time <= t && t < observation.end_us))
                        });
                        let start = issues.len();
                        let sample = FrameSample {
                            frame_index: observation.frame_index,
                            time_us: project
                                .canvas
                                .fps
                                .frame_time(observation.frame_index)
                                .unwrap(),
                            reasons: vec![],
                        };
                        crate::text_diagnostics::append(project, &sample, &error, issues);
                        // Geometry is constant throughout this evaluated interval.
                        for issue in &mut issues[start..] {
                            if issue.code == "TEXT_OVERFLOW" {
                                issue.end_us = Some(observation.end_us);
                            }
                        }
                    }
                    Some(error) => {
                        unavailable(
                            issues,
                            format!(
                                "文字 {} の区間を検査できません: {error}",
                                observation.clip_id
                            ),
                        );
                    }
                }
            }
            let skipped = result.total_intervals - attempted;
            if skipped > 0 {
                unavailable(
                    issues,
                    format!("期限または1000区間の上限により、文字の{skipped}区間は未検査です。"),
                );
            }
            if overflow > 0 {
                check.status = CheckStatus::Failed;
            }
            let margin = if margin_percent == 0 {
                "安全余白は未設定・未検査です。".into()
            } else {
                format!(
                    "上下左右{margin_percent}%の安全余白に{margin_warnings}区間の文字が入っています。画面/配置枠のはみ出し区間は安全余白の追加検査を省略しています。"
                )
            };
            check.description = format!(
                "文字の全{}描画区間中{}区間を正式フォント・描画処理で検査し、{}区間ではみ出しを検出。{} 重なり・読みやすさは未検査です。",
                result.total_intervals, inspected, overflow, margin
            );
        }
        Err(error) => unavailable(issues, format!("文字の全区間検査を実施できません: {error}")),
    }
    if let Err(error) = fs::remove_dir_all(&directory) {
        unavailable(
            issues,
            format!("文字検査の一時領域を片付けられません: {error}"),
        );
    }
    check
}
fn unavailable(issues: &mut Vec<Issue>, message: String) {
    // A bounded aggregate, including failures after the detailed issue limit.
    if let Some(issue) = issues.iter_mut().find(|i| i.code == "TEXT_NOT_INSPECTED") {
        issue.message = "一部の文字区間を検査できませんでした。実行環境・期限・区間上限を確認して再実行してください。".into();
    } else {
        issues.push(Issue {
            severity: Severity::Warning,
            code: "TEXT_NOT_INSPECTED".into(),
            field: "/clips".into(),
            clip_id: None,
            scene_id: None,
            asset_id: None,
            start_us: None,
            end_us: None,
            message,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn missing_font_is_explicit_and_owned_work_is_cleaned() {
        let root = std::env::temp_dir().join(format!("text-review-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let mut p: Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        p.clips[0].content = sungeki_project::ClipContent::Text {
            outline: None,
            layout: None,
            text: "字幕".into(),
            font_size: 20.,
            color: "#000000".into(),
        };
        let runtime = RenderRuntime {
            vrm: None,
            python: "/usr/bin/false".into(),
            image_worker: "/unused".into(),
            ffmpeg: "/unused".into(),
            ffprobe: "/unused".into(),
            font: root.join("missing"),
            font_sha256: "a".repeat(64),
            deadline: Duration::from_secs(5),
        };
        let mut issues = Vec::new();
        let check = inspect(&p, 0, &runtime, &root, &AtomicBool::new(false), &mut issues);
        assert_eq!(check.status, CheckStatus::NotRun);
        assert_eq!(issues[0].code, "TEXT_NOT_INSPECTED");
        assert!(!root.join("text-inspection").exists());
        fs::create_dir(root.join("text-inspection")).unwrap();
        fs::write(root.join("text-inspection/keep"), "preserved").unwrap();
        inspect(&p, 0, &runtime, &root, &AtomicBool::new(false), &mut issues);
        assert_eq!(
            fs::read_to_string(root.join("text-inspection/keep")).unwrap(),
            "preserved"
        );
        assert_eq!(issues.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
