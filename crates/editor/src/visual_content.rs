//! Typed visual content edits. Numeric/color validation is atomic at batch end.
use sungeki_project::{ClipContent, ShapeKind, ValidationError};
pub(crate) fn text_style(
    content: &mut ClipContent,
    size: f64,
    fill: &str,
) -> Result<(), ValidationError> {
    match content {
        ClipContent::Text {
            font_size, color, ..
        } => {
            *font_size = size;
            *color = fill.into();
            Ok(())
        }
        _ => Err(ValidationError::new(
            "文字以外のクリップへ文字設定を適用できません".into(),
        )),
    }
}
pub(crate) fn shape(
    content: &mut ClipContent,
    kind: &ShapeKind,
    color: &str,
) -> Result<(), ValidationError> {
    match content {
        ClipContent::Shape { shape, fill } => {
            *shape = kind.clone();
            *fill = color.into();
            Ok(())
        }
        _ => Err(ValidationError::new(
            "図形以外のクリップへ図形設定を適用できません".into(),
        )),
    }
}
pub(crate) fn blur(content: &mut ClipContent, value: f64) -> Result<(), ValidationError> {
    match content {
        ClipContent::Blur { sigma } => {
            *sigma = value;
            Ok(())
        }
        _ => Err(ValidationError::new("ぼかし以外には設定できません".into())),
    }
}

pub(crate) fn text_layout(
    content: &mut ClipContent,
    value: &Option<sungeki_project::SubtitleLayout>,
) -> Result<(), ValidationError> {
    match content {
        ClipContent::Text { layout, .. } => {
            *layout = value.clone();
            Ok(())
        }
        _ => Err(ValidationError::new(
            "文字以外のクリップへ文字組みを設定できません".into(),
        )),
    }
}

pub(crate) fn text_outline(
    content: &mut ClipContent,
    value: &Option<sungeki_project::SubtitleOutline>,
) -> Result<(), ValidationError> {
    match content {
        ClipContent::Text { outline, .. } => {
            *outline = value.clone();
            Ok(())
        }
        _ => Err(ValidationError::new(
            "文字以外のクリップへ縁取りを設定できません".into(),
        )),
    }
}
