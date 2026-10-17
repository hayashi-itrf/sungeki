use sungeki_project::*;
pub(super) fn fit(p: &Project, clip: &Clip, track: usize) -> Result<Transform, ValidationError> {
    let id = match &clip.content {
        ClipContent::Image { asset_id } | ClipContent::Psd { asset_id, .. } => asset_id.as_str(),
        ClipContent::Sprite { sprites, pose, .. } => sprites.asset_id(pose.as_deref(), false)?,
        _ => unreachable!(),
    };
    let probe = &p
        .assets
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| ValidationError::new("素材が見つかりません".into()))?
        .probe;
    let (width, height) = match probe {
        AssetProbe::Image(p) => {
            if (5..=8).contains(&p.orientation) {
                (p.height, p.width)
            } else {
                (p.width, p.height)
            }
        }
        AssetProbe::Psd(p) => (p.width, p.height),
        _ => return Err(ValidationError::new("画像の寸法を取得できません".into())),
    };
    let (w, h) = (width as f64, height as f64);
    if track == 0 {
        let scale = (1920. / w).max(1080. / h);
        let mut result = super::dialogue_template::rect(
            (1920. - w * scale) / 2.,
            (1080. - h * scale) / 2.,
            w * scale,
            h * scale,
        );
        result.flip_x = clip.transform.flip_x;
        Ok(result)
    } else {
        let scale = (350. / w).min(650. / h);
        let x = if track == 1 { 20. } else { 1550. };
        let mut result = super::dialogue_template::rect(
            x + (350. - w * scale) / 2.,
            790. - h * scale,
            w * scale,
            h * scale,
        );
        result.flip_x = super::standing_facing::inward(clip, track);
        Ok(result)
    }
}
