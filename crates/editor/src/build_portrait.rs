//! Portrait narration geometry; image aspect ratios and EXIF orientation are retained.
use crate::dialogue_template::rect;
use sungeki_project::*;

pub(crate) fn fit(
    p: &Project,
    clip: &Clip,
    background: bool,
) -> Result<Transform, ValidationError> {
    let asset_id = match &clip.content {
        ClipContent::Image { asset_id } | ClipContent::Psd { asset_id, .. } => asset_id.as_str(),
        ClipContent::Sprite { sprites, pose, .. } => sprites.asset_id(pose.as_deref(), false)?,
        _ => {
            return Err(ValidationError::new(
                "縦配置には画像またはPSDが必要です".into(),
            ));
        }
    };
    let probe = &p
        .assets
        .iter()
        .find(|a| a.id == asset_id)
        .ok_or_else(|| ValidationError::new("配置する素材が見つかりません".into()))?
        .probe;
    let (w, h) = match probe {
        AssetProbe::Image(a) if (5..=8).contains(&a.orientation) => (a.height, a.width),
        AssetProbe::Image(a) => (a.width, a.height),
        AssetProbe::Psd(a) => (a.width, a.height),
        _ => return Err(ValidationError::new("素材の画像寸法がありません".into())),
    };
    let (w, h) = (w as f64, h as f64);
    let mut transform = if background {
        let scale = (1080. / w).max(1920. / h);
        rect(
            (1080. - w * scale) / 2.,
            (1920. - h * scale) / 2.,
            w * scale,
            h * scale,
        )
    } else {
        let scale = (650. / w).min(680. / h);
        rect(
            (1080. - w * scale) / 2.,
            1440. - h * scale,
            w * scale,
            h * scale,
        )
    };
    transform.flip_x = clip.transform.flip_x;
    Ok(transform)
}
