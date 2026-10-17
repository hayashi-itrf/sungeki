use crate::scene_edit::{delta, first_start, group_clips, invalid, shifted};
use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::*;

type IdMap = BTreeMap<String, String>;
pub(crate) fn clip(
    p: &mut Project,
    id: &str,
    new_id: &str,
    start: TimeUs,
) -> Result<(), ValidationError> {
    let index = p
        .clips
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(invalid)?;
    let mut copy = p.clips[index].clone();
    copy.id = new_id.into();
    copy.start_us = start;
    // A same-time duplicate is a new layer. Keep its timing and scene membership.
    if p.clips.iter().any(|c| {
        c.track_id == copy.track_id
            && c.start_us.0 < copy.start_us.0.saturating_add(copy.duration_us.0)
            && copy.start_us.0 < c.start_us.0 + c.duration_us.0
    }) {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(format!("duplicate-track:{new_id}"));
        let mut bytes: [u8; 16] = digest[..16].try_into().unwrap();
        bytes[6] = (bytes[6] & 0x0f) | 0x80;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let track_index = p
            .tracks
            .iter()
            .position(|t| t.id == copy.track_id)
            .ok_or_else(invalid)?;
        let mut track = p.tracks[track_index].clone();
        track.id = uuid::Uuid::from_bytes(bytes).to_string();
        track.name = format!("{} (複製)", track.name);
        copy.track_id = track.id.clone();
        p.tracks.insert(track_index + 1, track);
    }
    // Insert directly above the original, retaining other clips' relative z-order.
    // Scene membership is preserved; a single copy does not join the original group.
    p.clips.insert(index + 1, copy);
    Ok(())
}
fn exact_ids(ids: impl Iterator<Item = String>, mapping: &IdMap) -> Result<(), ValidationError> {
    if ids.collect::<BTreeSet<_>>() != mapping.keys().cloned().collect() {
        return Err(invalid());
    }
    Ok(())
}
fn copy_clips(
    p: &Project,
    ids: &IdMap,
    delta: i64,
    scene: Option<&str>,
) -> Result<Vec<Clip>, ValidationError> {
    // Preserve source array order; the caller's ID map ordering has no rendering meaning.
    p.clips
        .iter()
        .filter(|c| ids.contains_key(&c.id))
        .map(|c| {
            let mut copy = c.clone();
            copy.id = ids[&c.id].clone();
            copy.start_us = shifted(c.start_us, delta)?;
            if let Some(scene) = scene {
                copy.scene_id = Some(scene.into());
            }
            Ok(copy)
        })
        .collect()
}
pub(crate) fn scene(
    p: &mut Project,
    id: &str,
    new_id: &str,
    start: TimeUs,
    clips: &IdMap,
    groups: &IdMap,
) -> Result<(), ValidationError> {
    if start.0 > MAX_TIME_US {
        return Err(invalid());
    }
    let mut copy = p
        .scenes
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(invalid)?;
    exact_ids(
        p.clips
            .iter()
            .filter(|c| c.scene_id.as_deref() == Some(id))
            .map(|c| c.id.clone()),
        clips,
    )?;
    let members: Vec<_> = p
        .groups
        .iter()
        .filter(|g| g.clip_ids.iter().any(|id| clips.contains_key(id)))
        .cloned()
        .collect();
    exact_ids(members.iter().map(|g| g.id.clone()), groups)?;
    let delta = delta(start, copy.start_us)?;
    let new_clips = copy_clips(p, clips, delta, Some(new_id))?;
    let new_groups = members
        .into_iter()
        .map(|mut g| {
            g.id = groups[&g.id].clone();
            g.clip_ids = g
                .clip_ids
                .iter()
                .map(|id| clips.get(id).cloned().ok_or_else(invalid))
                .collect::<Result<_, _>>()?;
            Ok(g)
        })
        .collect::<Result<Vec<_>, ValidationError>>()?;
    copy.id = new_id.into();
    copy.start_us = start;
    p.scenes.push(copy);
    p.clips.extend(new_clips);
    p.groups.extend(new_groups);
    Ok(())
}
pub(crate) fn group(
    p: &mut Project,
    id: &str,
    new_id: &str,
    start: TimeUs,
    clips: &IdMap,
) -> Result<(), ValidationError> {
    if start.0 > MAX_TIME_US {
        return Err(invalid());
    }
    let members = group_clips(p, id)?;
    exact_ids(members.iter().cloned(), clips)?;
    let delta = delta(start, first_start(p, &members)?)?;
    let new_clips = copy_clips(p, clips, delta, None)?;
    let mut copy = p
        .groups
        .iter()
        .find(|g| g.id == id)
        .cloned()
        .ok_or_else(invalid)?;
    copy.id = new_id.into();
    copy.clip_ids = members.iter().map(|id| clips[id].clone()).collect();
    p.groups.push(copy);
    p.clips.extend(new_clips);
    Ok(())
}
