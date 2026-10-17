use crate::{MAX_TIME_US, TimeUs, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Explicit, non-destructive BGM attenuation around audible speech windows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Ducking {
    pub attenuation_db: f64,
    pub attack_us: TimeUs,
    pub release_us: TimeUs,
}
impl Ducking {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !self.attenuation_db.is_finite()
            || !(-60.0..=0.0).contains(&self.attenuation_db)
            || self.attack_us.0 > MAX_TIME_US
            || self.release_us.0 > MAX_TIME_US
        {
            return Err(ValidationError::new(
                "ダッキングの音量・時間が不正です".into(),
            ));
        }
        Ok(())
    }
}
