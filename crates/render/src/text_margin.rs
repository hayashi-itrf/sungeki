//! The same layout in a translated inner canvas measures the configured safe
//! area without changing the text wrapping box, font, or production geometry.
use crate::{RenderError, TextLayoutIssue, execute::Runner, frame_cache::FrameCache};
use sungeki_project::Canvas;
use sungeki_scene::Layer;

pub(crate) fn inner(canvas: &Canvas, layer: &Layer, percent: u8) -> (Canvas, Layer, f64, f64) {
    let dx = (canvas.width * u32::from(percent)).div_ceil(100);
    let dy = (canvas.height * u32::from(percent)).div_ceil(100);
    let mut canvas = canvas.clone();
    canvas.width = canvas.width.saturating_sub(dx * 2);
    canvas.height = canvas.height.saturating_sub(dy * 2);
    let mut layer = layer.clone();
    layer.transform.x -= dx as f64;
    layer.transform.y -= dy as f64;
    (canvas, layer, dx as f64, dy as f64)
}
pub(crate) fn inspect(
    runner: &Runner<'_>,
    canvas: &Canvas,
    layer: &Layer,
    index: usize,
    frame: u64,
    percent: u8,
) -> Result<Vec<TextLayoutIssue>, RenderError> {
    if percent == 0 {
        return Ok(vec![]);
    }
    let (canvas, layer, dx, dy) = inner(canvas, layer, percent);
    if canvas.width == 0 || canvas.height == 0 {
        return Err(crate::plan::error(
            "指定した安全余白では内側の領域がありません",
        ));
    }
    match FrameCache::default().frame(runner, &canvas, &[&layer], index, frame) {
        Ok(_) => Ok(vec![]),
        Err(RenderError::TextLayout(mut issues)) => {
            // The original box already passed; only the inner canvas changed.
            if issues.iter().any(|i| !i.outside_canvas) {
                return Err(crate::plan::error("安全余白の診断が配置枠と一致しません"));
            }
            for issue in &mut issues {
                issue.bounds[0] += dx;
                issue.bounds[2] += dx;
                issue.bounds[1] += dy;
                issue.bounds[3] += dy;
            }
            Ok(issues)
        }
        Err(error) => Err(error),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translation_preserves_wrapping_and_maps_all_edges_to_the_inner_canvas() {
        let p: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let scene = sungeki_scene::ScenePlan::new(&p).unwrap();
        let layer = &scene.layers[0];
        let (inner, shifted, dx, dy) = super::inner(&scene.canvas, layer, 5);
        assert_eq!((inner.width, inner.height), (288, 162));
        assert_eq!((dx, dy), (16., 9.));
        assert_eq!(shifted.transform.width, layer.transform.width);
        assert_eq!(shifted.transform.height, layer.transform.height);
        assert_eq!(shifted.transform.x + dx, layer.transform.x);
        assert_eq!(shifted.transform.y + dy, layer.transform.y);
        assert_eq!(
            serde_json::to_value(shifted.primitive).unwrap(),
            serde_json::to_value(&layer.primitive).unwrap()
        );
    }
}
