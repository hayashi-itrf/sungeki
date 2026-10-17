use sungeki_project::{ClipContent, Project, ValidationError, VideoPlayback};
pub(crate) fn set(
    project: &mut Project,
    clip_id: &str,
    asset_id: &str,
    playback: &VideoPlayback,
) -> Result<(), ValidationError> {
    let clip = project
        .clips
        .iter_mut()
        .find(|c| c.id == clip_id)
        .ok_or_else(|| ValidationError::new("動画クリップが見つかりません".into()))?;
    let ClipContent::Video {
        asset_id: current,
        playback: settings,
    } = &mut clip.content
    else {
        return Err(ValidationError::new(
            "動画クリップを指定してください".into(),
        ));
    };
    *current = asset_id.to_owned();
    *settings = playback.clone();
    Ok(())
}
