use crate::{RenderError, plan::error};
use serde::Deserialize;
use serde_json::Value;
use sungeki_scene::{Layer, Primitive};
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextLayoutIssue {
    pub code: String,
    pub clip_id: String,
    pub bounds: [f64; 4],
    pub outside_canvas: bool,
}
/// Called only after frame/time/font/layout identity has been verified.
pub(crate) fn issues(
    value: &Value,
    layers: &[&Layer],
    canvas: &sungeki_project::Canvas,
) -> Result<Vec<TextLayoutIssue>, RenderError> {
    let issues: Vec<TextLayoutIssue> = serde_json::from_value(value.clone())?;
    if issues.len() > 10000 {
        return Err(error("文字診断の件数が上限を超えています"));
    }
    for issue in &issues {
        if issue.code != "TEXT_OVERFLOW"
            || !issue.bounds.iter().all(|v| v.is_finite())
            || issue.bounds[0] > issue.bounds[2]
            || issue.bounds[1] > issue.bounds[3]
            || !layers.iter().any(|layer| {
                layer.clip_id == issue.clip_id && matches!(layer.primitive, Primitive::Text { .. })
            })
        {
            return Err(error("文字診断と描画対象が一致しません"));
        }
        let layer = layers.iter().find(|l| l.clip_id == issue.clip_id).unwrap();
        let [left, top, right, bottom] = issue.bounds;
        let outside =
            left < 0.0 || top < 0.0 || right > canvas.width as f64 || bottom > canvas.height as f64;
        let t = &layer.transform;
        let outside_box =
            left < t.x || top < t.y || right > t.x + t.width || bottom > t.y + t.height;
        if issue.outside_canvas != outside || (!outside && !outside_box) {
            return Err(error("文字診断の範囲が描画計画と一致しません"));
        }
    }
    Ok(issues)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_target_bounds_and_worker_claim_before_exposing_text_issue() {
        let p: sungeki_project::Project =
            serde_json::from_str(include_str!("../../../tests/fixtures/psd-project.json")).unwrap();
        let plan = sungeki_scene::ScenePlan::new(&p).unwrap();
        let mut layer = plan.layers[0].clone();
        layer.primitive = Primitive::Text {
            layout: None,
            outline: None,
            text: "test".into(),
            font_size: 20.,
            color: "#000000".into(),
        };
        let value = serde_json::json!([{"code":"TEXT_OVERFLOW","clip_id":layer.clip_id,"bounds":[-1,0,10,10],"outside_canvas":true}]);
        assert_eq!(issues(&value, &[&layer], &plan.canvas).unwrap().len(), 1);
        for (key, wrong) in [
            ("clip_id", serde_json::json!("unknown")),
            ("code", serde_json::json!("unknown")),
            ("bounds", serde_json::json!([1, 1, 10, 10])),
            ("outside_canvas", serde_json::json!(false)),
        ] {
            let mut altered = value.clone();
            altered[0][key] = wrong;
            assert!(issues(&altered, &[&layer], &plan.canvas).is_err());
        }
        assert!(issues(&serde_json::json!(null), &[&layer], &plan.canvas).is_err());
        assert!(
            issues(&serde_json::json!([]), &[&layer], &plan.canvas)
                .unwrap()
                .is_empty()
        );
    }
}
