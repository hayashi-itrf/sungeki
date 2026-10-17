use crate::Command;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::{Project, ValidationError};

/// Generator-only planning: preserve clip times and the original back-to-front order.
/// Ordinary user edits are validated, never silently moved to another track.
pub(crate) fn separate(p: &mut Project) {
    let mut used: BTreeSet<String> = p
        .tracks
        .iter()
        .map(|t| t.id.clone())
        .chain(p.clips.iter().map(|c| c.id.clone()))
        .chain(p.scenes.iter().map(|s| s.id.clone()))
        .chain(p.groups.iter().map(|g| g.id.clone()))
        .chain(p.assets.iter().map(|a| a.id.clone()))
        .chain(p.characters.iter().map(|c| c.id.clone()))
        .collect();
    let mut by_track: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, clip) in p.clips.iter().enumerate() {
        by_track.entry(clip.track_id.clone()).or_default().push(i);
    }
    let mut tracks = Vec::new();
    for original in &p.tracks {
        let mut lanes = vec![original.clone()];
        let mut placed: Vec<(u64, u64, usize)> = Vec::new();
        for &index in by_track.get(&original.id).into_iter().flatten() {
            let clip = &mut p.clips[index];
            let start = clip.start_us.0;
            let end = start + clip.duration_us.0;
            let lane = placed
                .iter()
                .filter(|(a, b, _)| *a < end && start < *b)
                .map(|(_, _, lane)| lane + 1)
                .max()
                .unwrap_or(0);
            while lanes.len() <= lane {
                let mut salt = 0u64;
                let id = loop {
                    let digest = Sha256::digest(
                        serde_json::to_vec(&(
                            "vvm-exclusive-generator-track-v1",
                            &p.id,
                            &original.id,
                            &clip.id,
                            lanes.len(),
                            salt,
                        ))
                        .expect("serializable identifiers"),
                    );
                    let mut bytes: [u8; 16] = digest[..16].try_into().unwrap();
                    bytes[6] = (bytes[6] & 0x0f) | 0x80;
                    bytes[8] = (bytes[8] & 0x3f) | 0x80;
                    let id = uuid::Uuid::from_bytes(bytes).to_string();
                    if used.insert(id.clone()) {
                        break id;
                    }
                    salt += 1;
                };
                let mut track = original.clone();
                track.id = id;
                track.name = format!("{} ({})", original.name, lanes.len() + 1);
                lanes.push(track);
            }
            clip.track_id = lanes[lane].id.clone();
            placed.push((start, end, lane));
        }
        tracks.extend(lanes);
    }
    p.tracks = tracks;
}

pub(crate) fn complete_layout(
    p: &Project,
    commands: &mut Vec<Command>,
) -> Result<(), ValidationError> {
    let before = crate::transaction::candidate(p, commands)?;
    let mut separated = before.clone();
    separate(&mut separated);
    for track in &separated.tracks {
        if !before.tracks.iter().any(|t| t.id == track.id) {
            commands.push(Command::AddTrack {
                track: track.clone(),
            });
        }
    }
    for (old, new) in before.clips.iter().zip(&separated.clips) {
        if old.track_id != new.track_id {
            commands.push(Command::SetClipTrack {
                clip_id: new.id.clone(),
                track_id: new.track_id.clone(),
            });
        }
    }
    if before.tracks != separated.tracks {
        commands.push(Command::ReorderTracks {
            track_ids: separated.tracks.iter().map(|t| t.id.clone()).collect(),
        });
    }
    crate::track_collision::validate(&separated)
}
