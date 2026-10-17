use crate::{TimeUs, Transform, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum MotionEasing {
    Linear,
    EaseInOut,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VisualMotion {
    pub end_x: f64,
    pub end_y: f64,
    pub end_scale: f64,
    pub easing: MotionEasing,
}
impl VisualMotion {
    pub fn validate_parameters(&self) -> Result<(), ValidationError> {
        if ![self.end_x, self.end_y, self.end_scale]
            .iter()
            .all(|x| x.is_finite())
            || self.end_x.abs() > 1_000_000.
            || self.end_y.abs() > 1_000_000.
            || self.end_scale <= 0.
        {
            return Err(ValidationError::new(
                "画像の移動先・拡大率が描画可能な範囲外です".into(),
            ));
        }
        Ok(())
    }
    pub fn validate(&self, start: &Transform) -> Result<(), ValidationError> {
        self.validate_parameters()?;
        if start.width * self.end_scale > 32768.
            || start.height * self.end_scale > 32768.
            || start.width * self.end_scale < 0.000001
            || start.height * self.end_scale < 0.000001
        {
            return Err(ValidationError::new(
                "画像の移動先・拡大率が描画可能な範囲外です".into(),
            ));
        }
        Ok(())
    }
    pub fn sample(&self, start: &Transform, elapsed: TimeUs, duration: TimeUs) -> Transform {
        let p = (elapsed.0 as f64 / duration.0 as f64).clamp(0., 1.);
        let p = match self.easing {
            MotionEasing::Linear => p,
            MotionEasing::EaseInOut => 0.5 - 0.5 * (std::f64::consts::PI * p).cos(),
        };
        let mut result = start.clone();
        result.x = start.x + (self.end_x - start.x) * p;
        result.y = start.y + (self.end_y - start.y) * p;
        let scale = 1. + (self.end_scale - 1.) * p;
        result.width *= scale;
        result.height *= scale;
        result.motion = None;
        result
    }
}
