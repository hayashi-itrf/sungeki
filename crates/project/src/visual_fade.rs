use crate::{TimeUs, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VisualFade {
    pub in_us: TimeUs,
    pub out_us: TimeUs,
}
impl VisualFade {
    pub fn validate(&self, duration: TimeUs) -> Result<(), ValidationError> {
        if self
            .in_us
            .0
            .checked_add(self.out_us.0)
            .is_none_or(|v| v > duration.0)
        {
            return Err(ValidationError::new(
                "映像フェードの合計はクリップの長さ以内で指定してください".into(),
            ));
        }
        Ok(())
    }
    pub fn factor(&self, elapsed: TimeUs, duration: TimeUs) -> f64 {
        let fade_in = if self.in_us.0 == 0 {
            1.
        } else {
            elapsed.0 as f64 / self.in_us.0 as f64
        };
        let fade_out = if self.out_us.0 == 0 {
            1.
        } else {
            duration.0.saturating_sub(elapsed.0) as f64 / self.out_us.0 as f64
        };
        fade_in.min(fade_out).clamp(0., 1.)
    }
}
