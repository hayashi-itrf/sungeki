use std::collections::{BTreeMap, BTreeSet};
use sungeki_project::{Project, ValidationError};

pub(crate) fn properties(
    p: &mut Project,
    id: &str,
    name: &str,
    visible: bool,
    muted: bool,
) -> Result<(), ValidationError> {
    let track = p
        .tracks
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| ValidationError::new("編集対象のトラックが見つかりません".into()))?;
    track.name = name.into();
    track.visible = visible;
    track.muted = muted;
    Ok(())
}

pub(crate) fn reorder(p: &mut Project, ids: &[String]) -> Result<(), ValidationError> {
    let tracks: BTreeMap<_, _> = p.tracks.iter().map(|t| (&t.id, t)).collect();
    if ids.len() != tracks.len()
        || ids.iter().collect::<BTreeSet<_>>() != tracks.keys().copied().collect()
    {
        return Err(ValidationError::new(
            "並べ替えには全トラックのIDを重複なく指定してください".into(),
        ));
    }
    p.tracks = ids.iter().map(|id| tracks[id].clone()).collect();
    Ok(())
}
