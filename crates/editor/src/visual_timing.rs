use sungeki_project::*;

/// One-time alignment to the current timeline. No implicit link or speech mutation.
pub(crate) fn fit(
    project: &mut Project,
    clip_id: &str,
    first_id: &str,
    last_id: &str,
) -> Result<(), ValidationError> {
    let error = |message: &str| ValidationError::new(message.into());
    let speech = |id: &str| {
        project
            .clips
            .iter()
            .find(|clip| clip.id == id)
            .filter(|clip| matches!(clip.content, ClipContent::Speech { .. }))
            .ok_or_else(|| error("表示区間の基準となるセリフが見つかりません"))
    };
    let first = speech(first_id)?;
    let last = speech(last_id)?;
    if last.start_us < first.start_us {
        return Err(error("最後のセリフは最初のセリフ以降から選んでください"));
    }
    let start = first.start_us;
    let end = last
        .start_us
        .0
        .checked_add(last.duration_us.0)
        .filter(|end| *end <= MAX_TIME_US)
        .ok_or_else(|| error("表示区間が7日の範囲を超えます"))?;
    let first_end = first
        .start_us
        .0
        .checked_add(first.duration_us.0)
        .filter(|end| *end <= MAX_TIME_US)
        .ok_or_else(|| error("表示区間が7日の範囲を超えます"))?;
    if end < first_end {
        return Err(error("最後のセリフが最初のセリフより先に終わっています"));
    }
    let clip = project
        .clips
        .iter_mut()
        .find(|clip| clip.id == clip_id)
        .ok_or_else(|| error("表示区間を変更するクリップが見つかりません"))?;
    if !matches!(
        clip.content,
        ClipContent::Blur { .. }
            | ClipContent::Image { .. }
            | ClipContent::Psd { .. }
            | ClipContent::Sprite { .. }
            | ClipContent::Vrm { .. }
            | ClipContent::Text { .. }
            | ClipContent::Shape { .. }
    ) {
        return Err(error(
            "画像・立ち絵・文字・図形・ぼかしの表示区間を指定してください",
        ));
    }
    clip.start_us = start;
    clip.duration_us = TimeUs(end - start.0);
    // apply_batch validates scene bounds and fade durations atomically afterwards.
    Ok(())
}
