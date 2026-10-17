use std::collections::BTreeMap;
use sungeki_project::{Clip, Project, ValidationDiagnostic, ValidationError};

/// Check the completed transaction, allowing touching half-open intervals.
pub(crate) fn validate(project: &Project) -> Result<(), ValidationError> {
    validate_edit(&Project::empty(String::new(), String::new()), project)
}

/// Do not make unrelated edits to an existing document fail on untouched overlaps.
/// Every new or retimed overlap is rejected, regardless of the caller.
pub(crate) fn validate_edit(before: &Project, project: &Project) -> Result<(), ValidationError> {
    let original: BTreeMap<_, _> = before.clips.iter().map(|c| (c.id.as_str(), c)).collect();
    let unchanged = |c: &Clip| {
        original.get(c.id.as_str()).is_some_and(|old| {
            old.track_id == c.track_id
                && old.start_us == c.start_us
                && old.duration_us == c.duration_us
        })
    };
    let mut tracks: BTreeMap<&str, Vec<&Clip>> = BTreeMap::new();
    for clip in &project.clips {
        tracks.entry(&clip.track_id).or_default().push(clip);
    }
    for (id, mut clips) in tracks {
        clips.sort_by_key(|clip| clip.start_us.0);
        let mut active: Vec<&Clip> = Vec::new();
        for clip in clips {
            active.retain(|c| c.start_us.0 + c.duration_us.0 > clip.start_us.0);
            for previous in &active {
                if unchanged(previous) && unchanged(clip) {
                    continue;
                }
                let pair = [*previous, clip];
                let name = project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
                    .map(|track| track.name.as_str())
                    .unwrap_or(id);
                return Err(ValidationError::new(format!(
                    "トラック「{name}」内でクリップが重なります。開始時刻・長さを調整するか、別のトラックを指定してください。({} / {})",
                    pair[0].id, pair[1].id
                )).with_diagnostic(ValidationDiagnostic::TrackOverlap {
                    track_id: id.into(), track_name: name.into(),
                    first_clip_id: pair[0].id.clone(), second_clip_id: pair[1].id.clone(),
                }));
            }
            active.push(clip);
        }
    }
    Ok(())
}
