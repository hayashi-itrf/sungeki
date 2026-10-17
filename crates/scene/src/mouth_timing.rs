//! Shared, half-open mouth timing for PSD and sprite standings.
use sungeki_project::*;

pub(crate) fn spans(
    project: &Project,
    standing: &Clip,
    character: &str,
) -> Result<Vec<(TimeUs, TimeUs, bool)>, ValidationError> {
    let start = standing.start_us.0;
    let end = start
        .checked_add(standing.duration_us.0)
        .filter(|end| *end <= MAX_TIME_US)
        .ok_or_else(|| ValidationError::new("立ち絵の区間が不正です".into()))?;
    let muted: std::collections::HashSet<_> = project
        .tracks
        .iter()
        .filter(|t| t.muted)
        .map(|t| &t.id)
        .collect();
    let mut ranges = vec![];
    for clip in &project.clips {
        let ClipContent::Speech {
            character_id,
            audio,
            ..
        } = &clip.content
        else {
            continue;
        };
        let speech_end = clip.start_us.0 + clip.duration_us.0;
        if character_id != character
            || muted.contains(&clip.track_id)
            || speech_end <= start
            || clip.start_us.0 >= end
        {
            continue;
        }
        let timing = audio
            .as_ref()
            .and_then(|audio| audio.lip_sync.as_ref())
            .ok_or_else(|| ValidationError::new(format!("口パク解析が必要です: {}", clip.id)))?;
        for span in &timing.open {
            let absolute = |frame: u64| {
                clip.start_us.0
                    + (u128::from(frame) * 1_000_000).div_ceil(u128::from(timing.sample_rate))
                        as u64
            };
            let left = absolute(span.start_frame).max(start);
            let right = absolute(span.end_frame).min(speech_end).min(end);
            if left < right {
                ranges.push((left, right));
                if ranges.len() > 50_000 {
                    return Err(ValidationError::new(
                        "立ち絵の開口区間が50,000を超えています".into(),
                    ));
                }
            }
        }
    }
    // Overlapping same-character utterances use the union of open intervals.
    // The audio overlap itself remains a review issue; z-order is unchanged.
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = vec![];
    for (left, right) in ranges {
        if let Some(last) = merged.last_mut().filter(|last| left <= last.1) {
            last.1 = last.1.max(right);
        } else {
            merged.push((left, right));
        }
    }
    let mut result = vec![];
    let mut push = |left, right, open| {
        if left < right {
            result.push((TimeUs(left), TimeUs(right), open));
        }
    };
    let mut cursor = start;
    for (left, right) in merged {
        push(cursor, left, false);
        push(left, right, true);
        cursor = right;
    }
    push(cursor, end, false);
    Ok(result)
}
