use std::collections::HashSet;
use sungeki_project::*;

pub(crate) fn invalid() -> ValidationError {
    ValidationError::new("シーン・グループの対象または時刻が不正です".into())
}
pub(crate) fn delta(start: TimeUs, origin: TimeUs) -> Result<i64, ValidationError> {
    if start.0 > MAX_TIME_US || origin.0 > MAX_TIME_US {
        return Err(invalid());
    }
    Ok(start.0 as i64 - origin.0 as i64)
}
pub(crate) fn shifted(time: TimeUs, delta: i64) -> Result<TimeUs, ValidationError> {
    let next = i128::from(time.0) + i128::from(delta);
    if !(0..=i128::from(MAX_TIME_US)).contains(&next) {
        return Err(invalid());
    }
    Ok(TimeUs(next as u64))
}
pub(crate) fn scene<'a>(p: &'a mut Project, id: &str) -> Result<&'a mut Scene, ValidationError> {
    p.scenes.iter_mut().find(|s| s.id == id).ok_or_else(invalid)
}
pub(crate) fn group_clips(p: &Project, id: &str) -> Result<Vec<String>, ValidationError> {
    p.groups
        .iter()
        .find(|g| g.id == id)
        .map(|g| g.clip_ids.clone())
        .ok_or_else(invalid)
}
pub(crate) fn first_start(p: &Project, ids: &[String]) -> Result<TimeUs, ValidationError> {
    ids.iter()
        .map(|id| {
            p.clips
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.start_us)
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .ok_or_else(invalid)
}
pub(crate) fn move_selection(
    p: &mut Project,
    scenes: &[String],
    groups: &[String],
    clips: &[String],
    delta: i64,
) -> Result<(), ValidationError> {
    if delta.unsigned_abs() > MAX_TIME_US {
        return Err(invalid());
    }
    let scene_ids: HashSet<_> = scenes.iter().cloned().collect();
    let mut clip_ids: HashSet<_> = clips.iter().cloned().collect();
    for id in &scene_ids {
        scene(p, id)?;
    }
    for id in groups {
        clip_ids.extend(group_clips(p, id)?);
    }
    for clip in &p.clips {
        if clip
            .scene_id
            .as_ref()
            .is_some_and(|id| scene_ids.contains(id))
        {
            clip_ids.insert(clip.id.clone());
        }
    }
    for id in &clip_ids {
        if !p.clips.iter().any(|c| c.id == *id) {
            return Err(invalid());
        }
    }
    for s in &mut p.scenes {
        if scene_ids.contains(&s.id) {
            s.start_us = shifted(s.start_us, delta)?;
        }
    }
    for c in &mut p.clips {
        if clip_ids.contains(&c.id) {
            c.start_us = shifted(c.start_us, delta)?;
        }
    }
    Ok(())
}
pub(crate) fn move_scene(p: &mut Project, id: &str, start: TimeUs) -> Result<(), ValidationError> {
    if start.0 > MAX_TIME_US {
        return Err(invalid());
    }
    let delta = delta(start, scene(p, id)?.start_us)?;
    move_selection(p, &[id.into()], &[], &[], delta)
}
pub(crate) fn layout(
    p: &mut Project,
    ids: &[String],
    start: TimeUs,
    gap: TimeUs,
) -> Result<(), ValidationError> {
    if start.0 > MAX_TIME_US || gap.0 > MAX_TIME_US {
        return Err(invalid());
    }
    let mut seen = HashSet::new();
    let mut cursor = start;
    for (index, id) in ids.iter().enumerate() {
        if !seen.insert(id) {
            return Err(invalid());
        }
        let duration = scene(p, id)?.duration_us;
        move_scene(p, id, cursor)?;
        cursor = shifted(cursor, duration.0.try_into().map_err(|_| invalid())?)?;
        if index + 1 < ids.len() {
            cursor = shifted(cursor, gap.0 as i64)?;
        }
    }
    Ok(())
}
pub(crate) fn remove_scene(p: &mut Project, id: &str, delete: bool) -> Result<(), ValidationError> {
    scene(p, id)?;
    if delete {
        let removed: HashSet<_> = p
            .clips
            .iter()
            .filter(|c| c.scene_id.as_deref() == Some(id))
            .map(|c| c.id.clone())
            .collect();
        p.clips.retain(|c| !removed.contains(&c.id));
        for g in &mut p.groups {
            g.clip_ids.retain(|id| !removed.contains(id));
        }
        p.groups.retain(|g| !g.clip_ids.is_empty());
    } else {
        for c in &mut p.clips {
            if c.scene_id.as_deref() == Some(id) {
                c.scene_id = None;
            }
        }
    }
    p.scenes.retain(|s| s.id != id);
    Ok(())
}
