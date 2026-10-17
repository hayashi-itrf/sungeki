//! Bounded formal renders for review. Successful PNG generation is not visual approval.
use crate::{Check, CheckKind, CheckStatus, FrameSample, FrameSampling, plan_review_frames};
use serde::Serialize;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use sungeki_project::{Project, TimeUs};
use sungeki_render::{FrameArtifact, FramePlan, RenderRuntime, render_frame_staged};

#[derive(Debug, Serialize)]
pub struct ReviewedFrame {
    pub sample: FrameSample,
    pub output: Option<FrameArtifact>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct RepresentativeFrames {
    /// Merged into the host report diagnostics; not duplicated in image artifacts.
    #[serde(skip)]
    pub text_issues: Vec<crate::Issue>,
    #[serde(skip)]
    pub text_check: Check,
    pub sampling: FrameSampling,
    pub frames: Vec<ReviewedFrame>,
    pub contact_sheets: Vec<crate::ContactSheet>,
    pub check: Check,
    pub visual_review: Check,
}
#[derive(Debug)]
pub enum RepresentativeFrameError {
    Invalid(String),
    Io(std::io::Error),
    Cancelled,
}
impl From<std::io::Error> for RepresentativeFrameError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
/// Takes a new empty absolute work directory. Per-frame failures are retained and
/// later frames continue; cancellation is a job interruption, not a passing report.
/// Originals are verified against the same Project hashes separately for each frame.
pub fn review_representative_frames(
    project: &Project,
    requested: &[TimeUs],
    limit: usize,
    text_safe_margin_percent: u8,
    runtime: &RenderRuntime,
    directory: &Path,
    cancelled: &AtomicBool,
) -> Result<RepresentativeFrames, RepresentativeFrameError> {
    if text_safe_margin_percent > 25 {
        return Err(RepresentativeFrameError::Invalid(
            "安全余白は0〜25%で指定してください".into(),
        ));
    }
    let sampling = plan_review_frames(project, requested, limit)
        .map_err(|e| RepresentativeFrameError::Invalid(e.0))?;
    if cancelled.load(Ordering::SeqCst) {
        return Err(RepresentativeFrameError::Cancelled);
    }
    if !directory.is_absolute()
        || !fs::symlink_metadata(directory)?.is_dir()
        || fs::read_dir(directory)?.next().is_some()
    {
        return Err(RepresentativeFrameError::Invalid(
            "空の専用作業ディレクトリが必要です".into(),
        ));
    }
    if runtime.deadline.is_zero() || runtime.deadline.as_secs_f64() > 86400.0 {
        return Err(RepresentativeFrameError::Invalid(
            "描画期限が不正です".into(),
        ));
    }
    let end = Instant::now() + runtime.deadline;
    let mut frames = Vec::new();
    let mut text_issues = Vec::new();
    for sample in &sampling.selected {
        if cancelled.load(Ordering::SeqCst) {
            return Err(RepresentativeFrameError::Cancelled);
        }
        let remaining = end.saturating_duration_since(Instant::now());
        let rendered = if remaining.is_zero() {
            Err(sungeki_render::RenderError::Message(
                "代表画像全体の実行期限を超えたため未生成です".into(),
            ))
        } else {
            render_one(
                project,
                sample,
                &RenderRuntime {
                    deadline: remaining,
                    ..runtime.clone()
                },
                directory,
                cancelled,
            )
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err(RepresentativeFrameError::Cancelled);
        }
        let rendered = rendered.and_then(|output| {
            if Instant::now() >= end {
                Err(sungeki_render::RenderError::Message(
                    "代表画像全体の実行期限を超えました".into(),
                ))
            } else {
                Ok(output)
            }
        });
        let (output, error) = match rendered {
            Ok(output) => (Some(output), None),
            Err(error) => {
                crate::text_diagnostics::append(project, sample, &error, &mut text_issues);
                (None, Some(error.to_string()))
            }
        };
        frames.push(ReviewedFrame {
            sample: sample.clone(),
            output,
            error,
        });
    }
    let mut contact_sheets = Vec::new();
    let pages = frames.len().div_ceil(crate::contact_sheet::PAGE_SIZE);
    for (index, page_frames) in frames.chunks(crate::contact_sheet::PAGE_SIZE).enumerate() {
        if cancelled.load(Ordering::SeqCst) {
            return Err(RepresentativeFrameError::Cancelled);
        }
        let remaining = end.saturating_duration_since(Instant::now());
        let rendered = if remaining.is_zero() {
            Err("画像一覧の実行期限を超えました".into())
        } else {
            crate::contact_sheet::render_page(
                page_frames,
                index + 1,
                pages,
                &RenderRuntime {
                    deadline: remaining,
                    ..runtime.clone()
                },
                directory,
                cancelled,
            )
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err(RepresentativeFrameError::Cancelled);
        }
        let rendered = rendered.and_then(|output| {
            if Instant::now() >= end {
                Err("代表画像全体の実行期限を超えました".into())
            } else {
                Ok(output)
            }
        });
        let (output, error) = match rendered {
            Ok(output) => (Some(output), None),
            Err(error) => (None, Some(error)),
        };
        contact_sheets.push(crate::ContactSheet {
            page: index + 1,
            frame_indices: page_frames
                .iter()
                .map(|frame| frame.sample.frame_index)
                .collect(),
            output,
            error,
        });
    }
    let mut text_check = Check {
        kind: CheckKind::TextLayout,
        status: CheckStatus::NotRun,
        description: "代表画像だけでは全字幕・安全余白は未検査です。".into(),
    };
    let remaining = end.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        text_check = crate::text_inspection::inspect(
            project,
            text_safe_margin_percent,
            &RenderRuntime {
                deadline: remaining,
                ..runtime.clone()
            },
            directory,
            cancelled,
            &mut text_issues,
        );
    }
    if text_issues.iter().any(|i| i.code == "TEXT_OVERFLOW") {
        text_check.status = CheckStatus::Failed;
    }
    if cancelled.load(Ordering::SeqCst) {
        return Err(RepresentativeFrameError::Cancelled);
    }
    let failed = frames.iter().filter(|frame| frame.output.is_none()).count();
    let sheet_failures = contact_sheets
        .iter()
        .filter(|page| page.output.is_none())
        .count();
    let status = if frames.is_empty() {
        CheckStatus::NotRun
    } else if failed > 0 || sheet_failures > 0 {
        CheckStatus::Failed
    } else {
        CheckStatus::Passed
    };
    let check = Check {
        kind: CheckKind::RepresentativeFrames,
        status,
        description: format!(
            "代表画像{}枚中{}枚を生成。候補の省略{}枚。画像を抜き出していない区間{}か所。画像一覧{}ページ中{}ページ生成。全編の検査ではありません。",
            frames.len(),
            frames.len() - failed,
            sampling.omitted.len(),
            sampling.unsampled.len(),
            pages,
            pages - sheet_failures
        ),
    };
    Ok(RepresentativeFrames { text_check, text_issues, sampling, frames, contact_sheets, check, visual_review: Check {
        kind: CheckKind::VisualReview, status: CheckStatus::NotRun,
        description: "人間または画像を読めるAIによる内容確認は未実施です。画像生成の成功は映像内容の合格を意味しません。".into(),
    } })
}
fn render_one(
    project: &Project,
    sample: &FrameSample,
    runtime: &RenderRuntime,
    directory: &Path,
    cancelled: &AtomicBool,
) -> Result<FrameArtifact, sungeki_render::RenderError> {
    use std::os::unix::fs::DirBuilderExt;
    let plan = FramePlan::new(project, sample.frame_index)?;
    let work = directory.join(format!("frame-{:012}", sample.frame_index));
    fs::DirBuilder::new().mode(0o700).create(&work)?;
    render_frame_staged(&plan, runtime, &work, cancelled)
}
