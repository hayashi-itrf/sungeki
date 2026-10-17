use crate::{Check, CheckKind, CheckStatus, Diagnostics, Issue, Severity};
use sungeki_project::Project;

/// Runs the same invariant validator as editing/loading. No filesystem access,
/// rendering, media probing, guessed text metrics, or Project mutation occurs.
pub fn review_structure(project: &Project) -> Diagnostics {
    let validation = sungeki_project::validate(project);
    let mut issues = Vec::new();
    let status = match validation {
        Ok(()) => CheckStatus::Passed,
        Err(error) => {
            issues.push(Issue {
                severity: Severity::Error,
                code: "PROJECT_INVALID".into(),
                field: "/".into(),
                clip_id: None,
                scene_id: None,
                asset_id: None,
                start_us: None,
                end_us: None,
                message: error.0,
            });
            CheckStatus::Failed
        }
    };
    let mut checks = vec![Check {
        kind: CheckKind::Structure,
        status,
        description: "素材やクリップの参照、所属、時刻、設定値の確認。問題がある場合は最初の1件を報告します。".into(),
    }];
    for (kind, description) in [
        (
            CheckKind::AssetIntegrity,
            "元ファイルの存在と内容は未検査です。登録情報だけでは原本を確認したことにはなりません。",
        ),
        (
            CheckKind::TextLayout,
            "動画に書き出す際の文字組みによる、字幕のはみ出しや画面端との余白は未検査です。",
        ),
        (CheckKind::Audio, "音声の重なり・切断・音量は未検査です。"),
        (
            CheckKind::RepresentativeFrames,
            "動画から抜き出した確認用画像とその一覧は未生成です。全編の映像は確認していません。",
        ),
        (
            CheckKind::VisualReview,
            "人間または画像を読めるAIによる映像内容の確認は未実施です。",
        ),
    ] {
        checks.push(Check {
            kind,
            status: CheckStatus::NotRun,
            description: description.into(),
        });
    }
    Diagnostics::new(checks, issues)
}
