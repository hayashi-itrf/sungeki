//! Explicit boundary-based movement; never trim audio or expand selection implicitly.
use std::collections::HashSet;
use sungeki_project::{MAX_TIME_US, Project, TimeUs, ValidationError};

pub(crate) fn apply(
    project: &mut Project,
    clip_ids: &[String],
    boundary_us: TimeUs,
    delta_us: i64,
) -> Result<(), ValidationError> {
    if boundary_us.0 > MAX_TIME_US || delta_us.unsigned_abs() > MAX_TIME_US {
        return Err(ValidationError::new(
            "移動境界または移動量が範囲外です".into(),
        ));
    }
    let targets: HashSet<_> = clip_ids.iter().map(String::as_str).collect();
    let mut found = 0;
    let mut moves = Vec::new();
    for (index, clip) in project.clips.iter().enumerate() {
        if !targets.contains(clip.id.as_str()) {
            continue;
        }
        found += 1;
        let end = clip
            .start_us
            .0
            .checked_add(clip.duration_us.0)
            .ok_or_else(|| ValidationError::new("クリップ終端が範囲外です".into()))?;
        if clip.start_us < boundary_us && end > boundary_us.0 {
            return Err(ValidationError::new(
                "移動境界をまたぐクリップがあります。境界または対象を変更してください".into(),
            ));
        }
        if clip.start_us >= boundary_us {
            moves.push((index, crate::scene_edit::shifted(clip.start_us, delta_us)?));
        }
    }
    if found != targets.len() {
        return Err(ValidationError::new(
            "移動対象のクリップが見つかりません".into(),
        ));
    }
    for (index, start) in moves {
        project.clips[index].start_us = start;
    }
    // Whole-batch validation checks scene bounds and final end times, allowing an
    // explicit ResizeScene in this same batch without moving any unselected clips.
    Ok(())
}
