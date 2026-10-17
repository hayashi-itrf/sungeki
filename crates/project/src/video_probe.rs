//! Serializable source identity and clocks. No filesystem or decoder calls.
use crate::{MediaRatio, MediaStreamClock, TimeUs, ValidationError, VideoMetadata};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct VideoProbe {
    pub sha256: String,
    #[ts(type = "number")]
    pub byte_length: u64,
    pub metadata: VideoMetadata,
}
fn invalid() -> ValidationError {
    ValidationError::new("動画原本の情報が不正です".into())
}
impl MediaRatio {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.numerator == 0 || self.denominator == 0 {
            return Err(invalid());
        }
        Ok(())
    }
}
impl MediaStreamClock {
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.time_base.validate()?;
        if self
            .start_pts
            .is_some_and(|v| v.unsigned_abs() > MAX_SAFE_INTEGER)
            || self
                .duration_ts
                .is_some_and(|v| v == 0 || v > MAX_SAFE_INTEGER)
        {
            return Err(invalid());
        }
        Ok(())
    }
    /// Rounded upward for an enclosing interval. Unknown or out-of-project-range
    /// durations stay unavailable; registration must not invent a default length.
    pub fn duration_us(&self) -> Result<Option<TimeUs>, ValidationError> {
        self.validate()?;
        let Some(ticks) = self.duration_ts else {
            return Ok(None);
        };
        let us = (u128::from(ticks) * u128::from(self.time_base.numerator) * 1_000_000)
            .div_ceil(u128::from(self.time_base.denominator));
        if us > u128::from(crate::MAX_TIME_US) {
            return Err(invalid());
        }
        Ok(Some(TimeUs(us as u64)))
    }
}
impl VideoProbe {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || self.byte_length == 0
            || self.byte_length > 4 * 1024 * 1024 * 1024
        {
            return Err(invalid());
        }
        self.metadata.validate()
    }
}
impl VideoMetadata {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let video = &self.video;
        let codec = |s: &str| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control);
        if !codec(&video.codec)
            || !(1..=16384).contains(&video.width)
            || !(1..=16384).contains(&video.height)
            || u64::from(video.width) * u64::from(video.height) > 100_000_000
            || video
                .frame_count
                .is_some_and(|n| n == 0 || n > MAX_SAFE_INTEGER)
            || video
                .rotation_degrees
                .is_some_and(|v| !v.is_finite() || v.abs() > 3600.0)
            || video
                .display_matrix
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 4096 || v.contains('\0'))
            || video
                .legacy_rotate_tag
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 128 || v.chars().any(char::is_control))
            || self.audio.len() > 63
        {
            return Err(invalid());
        }
        video.clock.validate()?;
        for ratio in [
            &video.sample_aspect_ratio,
            &video.average_frame_rate,
            &video.nominal_frame_rate,
        ]
        .into_iter()
        .flatten()
        {
            ratio.validate()?;
        }
        let mut ids = std::collections::HashSet::from([video.index]);
        for audio in &self.audio {
            if !ids.insert(audio.index)
                || !codec(&audio.codec)
                || !(1..=768000).contains(&audio.sample_rate)
                || !(1..=64).contains(&audio.channels)
            {
                return Err(invalid());
            }
            audio.clock.validate()?;
        }
        Ok(())
    }
}
