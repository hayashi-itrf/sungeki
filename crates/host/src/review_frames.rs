#[path = "review_mix.rs"]
mod mix;
use crate::review_record::{Input, invalid};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    sync::atomic::{AtomicBool, Ordering},
};
use sungeki_project::{Check, CheckKind, CheckStatus, Diagnostics};
use sungeki_protocol::{ApiError, ReviewFrames, ReviewReport};

pub(crate) fn complete(
    input: &Input,
    cancelled: &AtomicBool,
    mut report: ReviewReport,
) -> Result<Option<ReviewReport>, ApiError> {
    if cancelled.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let started = std::time::Instant::now();
    let check = if let Some(visual) = &input.visual {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&visual.directory)
            .map_err(invalid)?;
        fs::File::open(
            visual
                .directory
                .parent()
                .ok_or_else(|| invalid("作業領域の親がありません"))?,
        )
        .and_then(|file| file.sync_all())
        .map_err(invalid)?;
        match sungeki_review::review_representative_frames(
            &input.project,
            &input.sampling.requested_times_us,
            input.sampling.max_frames as usize,
            input.sampling.text_safe_margin_percent.unwrap_or(0),
            &visual.runtime,
            &visual.directory,
            cancelled,
        ) {
            Ok(mut output) => {
                for check in &mut report.diagnostics.checks {
                    if check.kind == CheckKind::TextLayout {
                        *check = output.text_check.clone();
                    }
                }
                report.diagnostics.issues.append(&mut output.text_issues);
                // Strict protocol decoding makes a core/transport shape drift an explicit failure.
                let value: ReviewFrames =
                    serde_json::from_value(serde_json::to_value(output).map_err(invalid)?)
                        .map_err(invalid)?;
                let check = value.check.clone();
                report.representative_frames = Some(value);
                fs::File::open(&visual.directory)
                    .and_then(|file| file.sync_all())
                    .map_err(invalid)?;
                check
            }
            Err(sungeki_review::RepresentativeFrameError::Cancelled) => return Ok(None),
            Err(sungeki_review::RepresentativeFrameError::Invalid(message)) => Check {
                kind: CheckKind::RepresentativeFrames,
                status: CheckStatus::Failed,
                description: message,
            },
            Err(error) => return Err(invalid(error)),
        }
    } else {
        Check {
            kind: CheckKind::RepresentativeFrames,
            status: CheckStatus::NotRun,
            description: format!(
                "代表画像は未生成です。{}",
                input
                    .visual_unavailable
                    .as_deref()
                    .unwrap_or("描画環境が記録されていません")
            ),
        }
    };
    if cancelled.load(Ordering::SeqCst) {
        return Ok(None);
    }
    mix::inspect(input, started, cancelled, &mut report);
    if cancelled.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let checks = report
        .diagnostics
        .checks
        .into_iter()
        .map(|old| {
            if old.kind == CheckKind::RepresentativeFrames {
                check.clone()
            } else {
                old
            }
        })
        .collect();
    report.diagnostics = Diagnostics::new(checks, report.diagnostics.issues);
    Ok(Some(report))
}

pub(crate) fn validate(
    input: &Input,
    job: &sungeki_storage::StoredJob,
    state: &crate::review_record::State,
) -> Result<(), ApiError> {
    if input
        .sampling
        .text_safe_margin_percent
        .is_some_and(|v| v > 25)
        || !(1..=120).contains(&input.sampling.max_frames)
        || input.sampling.requested_times_us.len() > 120
    {
        return Err(invalid("代表画像の選定条件が不正です"));
    }
    if let Some(visual) = &input.visual {
        if !visual.directory.is_absolute()
            || visual.directory.file_name().and_then(|s| s.to_str())
                != Some(format!("{}.frames", job.id).as_str())
        {
            return Err(invalid("代表画像の作業領域が不正です"));
        }
    }
    let Some(report) = &state.report else {
        return Ok(());
    };
    let Some(frames) = &report.representative_frames else {
        return Ok(());
    };
    let planned = sungeki_review::plan_review_frames(
        &input.project,
        &input.sampling.requested_times_us,
        input.sampling.max_frames as usize,
    )
    .map_err(invalid)?;
    if serde_json::to_value(planned).map_err(invalid)?
        != serde_json::to_value(&frames.sampling).map_err(invalid)?
    {
        return Err(invalid("代表画像の選定結果が要求と一致しません"));
    }
    let visual = input
        .visual
        .as_ref()
        .ok_or_else(|| invalid("代表画像の入力がありません"))?;
    if state.phase != sungeki_project::JobPhase::Succeeded
        || state.error.is_some()
        || report.report_id != job.id
        || report.content_hash != input.content_hash
        || report.source_revision != input.source_revision
        || frames.frames.len() > input.sampling.max_frames as usize
        || frames.frames.len() != frames.sampling.selected.len()
        || frames.contact_sheets.len() != frames.frames.len().div_ceil(12)
        || frames.check.kind != CheckKind::RepresentativeFrames
        || frames.visual_review.kind != CheckKind::VisualReview
        || frames.visual_review.status != CheckStatus::NotRun
    {
        return Err(invalid("代表画像レポートと開始時点が一致しません"));
    }
    let hash_ok = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    };
    let check_output = |output: &sungeki_protocol::FramePreviewOutput,
                        path: std::path::PathBuf,
                        frame: u64,
                        time: sungeki_project::TimeUs,
                        width: u32,
                        height: u32|
     -> Result<(), ApiError> {
        if path.to_str() != Some(output.path.as_str())
            || output.frame_index != frame
            || output.time_us != time
            || output.width != width
            || output.height != height
            || !hash_ok(&output.sha256)
            || output.byte_length == 0
            || output.byte_length > 512 * 1024 * 1024
            || output.font_sha256 != visual.runtime.font_sha256
            || !hash_ok(&output.font_sha256)
            || !matches!(output.layout_version, 7 | 8 | 9)
        {
            return Err(invalid("代表画像の成果物記録が不正です"));
        }
        Ok(())
    };
    let count = input.project.duration().and_then(|duration| {
        sungeki_scene::first_frame_at_or_after(&input.project.canvas.fps, duration)
    });
    if count != Some(frames.sampling.frame_count) {
        return Err(invalid("代表画像の総frame数が不正です"));
    }
    let mut expected_unsampled = Vec::new();
    let mut first = 0;
    for sample in &frames.sampling.selected {
        if first < sample.frame_index {
            expected_unsampled.push((first, sample.frame_index));
        }
        first = sample.frame_index.saturating_add(1);
    }
    if first < frames.sampling.frame_count {
        expected_unsampled.push((first, frames.sampling.frame_count));
    }
    if expected_unsampled
        != frames
            .sampling
            .unsampled
            .iter()
            .map(|span| (span.first, span.end))
            .collect::<Vec<_>>()
    {
        return Err(invalid("代表画像の未抽出区間が不正です"));
    }
    let mut previous = None;
    for (frame, sample) in frames.frames.iter().zip(&frames.sampling.selected) {
        if &frame.sample != sample
            || frame.output.is_some() == frame.error.is_some()
            || sample.frame_index >= frames.sampling.frame_count
            || previous.is_some_and(|v| v >= sample.frame_index)
            || input.project.canvas.fps.frame_time(sample.frame_index) != Some(sample.time_us)
        {
            return Err(invalid("代表画像の抽出時刻が不正です"));
        }
        previous = Some(sample.frame_index);
        if let Some(output) = &frame.output {
            check_output(
                output,
                visual
                    .directory
                    .join(format!("frame-{:012}/frame-0.png", sample.frame_index)),
                sample.frame_index,
                sample.time_us,
                input.project.canvas.width,
                input.project.canvas.height,
            )?;
        }
    }
    for (index, page) in frames.contact_sheets.iter().enumerate() {
        let part = &frames.frames[index * 12..((index + 1) * 12).min(frames.frames.len())];
        if page.page as usize != index + 1
            || page.output.is_some() == page.error.is_some()
            || page.frame_indices
                != part
                    .iter()
                    .map(|f| f.sample.frame_index)
                    .collect::<Vec<_>>()
        {
            return Err(invalid("画像一覧のページ記録が不正です"));
        }
        if let Some(output) = &page.output {
            check_output(
                output,
                visual
                    .directory
                    .join(format!("contact-sheet-{:03}/frame-0.png", page.page)),
                0,
                sungeki_project::TimeUs(0),
                1216,
                (116 + part.len().div_ceil(3) * 300) as u32,
            )?;
        }
    }
    let expected = if frames.frames.is_empty() {
        CheckStatus::NotRun
    } else if frames.frames.iter().any(|f| f.output.is_none())
        || frames.contact_sheets.iter().any(|p| p.output.is_none())
    {
        CheckStatus::Failed
    } else {
        CheckStatus::Passed
    };
    if frames.check.status != expected
        || !report
            .diagnostics
            .checks
            .iter()
            .any(|c| c.kind == CheckKind::RepresentativeFrames && c.status == expected)
    {
        return Err(invalid("代表画像の検査状態が一致しません"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "review_frames_tests.rs"]
mod tests;
