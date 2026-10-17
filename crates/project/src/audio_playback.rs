use crate::{AudioProbe, MAX_TIME_US, TimeUs, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Non-destructive selection of an original. Fades span the clip, not each loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AudioPlayback {
    pub source_start_us: TimeUs,
    pub source_duration_us: TimeUs,
    pub looped: bool,
    pub gain_db: f64,
    #[serde(default)]
    pub volume_points: Vec<crate::VolumePoint>,
    pub fade_in_us: TimeUs,
    pub fade_out_us: TimeUs,
    pub ducking: Option<crate::Ducking>,
}

impl AudioPlayback {
    pub fn validate(&self, probe: &AudioProbe, duration: TimeUs) -> Result<(), ValidationError> {
        probe.validate()?;
        crate::volume_point::validate_points(&self.volume_points, duration)?;
        if let Some(ducking) = &self.ducking {
            ducking.validate()?;
        }
        let end = self
            .source_start_us
            .0
            .checked_add(self.source_duration_us.0);
        let fades = self.fade_in_us.0.checked_add(self.fade_out_us.0);
        if duration.0 == 0
            || duration.0 > MAX_TIME_US
            || self.source_duration_us.0 == 0
            || end.is_none_or(|end| end > probe.duration_us.0)
            || (!self.looped && duration > self.source_duration_us)
            || !self.gain_db.is_finite()
            || !(-60.0..=24.0).contains(&self.gain_db)
            || fades.is_none_or(|sum| sum > duration.0)
        {
            return Err(ValidationError::new(
                "音声の使用区間・繰り返し・音量・フェードが不正です".into(),
            ));
        }
        // A microsecond interval must contain at least one actual source sample.
        let first = (u128::from(self.source_start_us.0) * u128::from(probe.sample_rate))
            .div_ceil(1_000_000);
        let last = (u128::from(end.unwrap()) * u128::from(probe.sample_rate))
            .div_ceil(1_000_000)
            .min(u128::from(probe.frames));
        if first >= last {
            return Err(ValidationError::new(
                "音声の使用区間にサンプルがありません".into(),
            ));
        }
        Ok(())
    }
}
