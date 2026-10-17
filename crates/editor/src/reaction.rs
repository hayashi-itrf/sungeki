//! A reaction is an ordinary image with center-preserving growth and alpha fade.
//! Absolute final geometry makes repeating the command idempotent.
use sungeki_project::*;

pub(crate) fn set(
    clip: &mut Clip,
    center_x: f64,
    center_y: f64,
    width: f64,
    height: f64,
    start_scale: f64,
    fade: &VisualFade,
) -> Result<(), ValidationError> {
    if !matches!(clip.content, ClipContent::Image { .. }) {
        return Err(ValidationError::new(
            "リアクションには画像クリップを指定してください".into(),
        ));
    }
    if ![center_x, center_y, width, height, start_scale]
        .iter()
        .all(|v| v.is_finite())
        || width <= 0.
        || height <= 0.
        || start_scale <= 0.
        || start_scale > 1.
    {
        return Err(ValidationError::new(
            "リアクションの寸法・開始倍率が不正です".into(),
        ));
    }
    fade.validate(clip.duration_us)?;
    let mut transform = clip.transform.clone();
    transform.width = width * start_scale;
    transform.height = height * start_scale;
    transform.x = center_x - transform.width / 2.;
    transform.y = center_y - transform.height / 2.;
    transform.motion = Some(VisualMotion {
        end_x: center_x - width / 2.,
        end_y: center_y - height / 2.,
        end_scale: 1. / start_scale,
        easing: MotionEasing::EaseInOut,
    });
    transform.motion.as_ref().unwrap().validate(&transform)?;
    transform.fade = Some(fade.clone());
    clip.transform = transform;
    Ok(())
}
