use crate::{AudioProbe, ValidationError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const LIPSYNC_ANALYSIS_VERSION: u32 = 1;
pub const MAX_LIPSYNC_WINDOWS: u64 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LipSyncSettings {
    pub window_us: u32,
    // Absolute full-scale RMS floor, in millionths (10_000 = 0.01).
    pub floor_ppm: u32,
    // Fraction of this recording's peak window RMS, in millionths.
    pub peak_ratio_ppm: u32,
    pub min_open_us: u32,
    // Fill only internal closed gaps shorter than this duration.
    pub min_close_us: u32,
}
impl Default for LipSyncSettings {
    fn default() -> Self {
        Self {
            window_us: 20_000,
            floor_ppm: 10_000,
            peak_ratio_ppm: 150_000,
            min_open_us: 60_000,
            min_close_us: 80_000,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MouthInterval {
    // Half-open sample-frame range relative to the source WAV.
    #[ts(type = "number")]
    pub start_frame: u64,
    #[ts(type = "number")]
    pub end_frame: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct LipSyncAnalysis {
    pub version: u32,
    pub source_sha256: String,
    pub settings: LipSyncSettings,
    pub sample_rate: u32,
    #[ts(type = "number")]
    pub source_frames: u64,
    pub peak_rms: f64,
    pub threshold_rms: f64,
    pub open: Vec<MouthInterval>,
}
impl LipSyncAnalysis {
    /// Source-relative integer time, independent of render FPS and query order.
    /// At the source end and in unvoiced gaps the mouth is closed.
    pub fn is_open_at(&self, time_us: u64) -> bool {
        let frame = u128::from(time_us) * u128::from(self.sample_rate) / 1_000_000;
        if frame >= u128::from(self.source_frames) {
            return false;
        }
        let frame = frame as u64;
        let next = self.open.partition_point(|span| span.end_frame <= frame);
        self.open
            .get(next)
            .is_some_and(|span| span.start_frame <= frame)
    }
}

impl LipSyncSettings {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !(10_000..=100_000).contains(&self.window_us)
            || !(1..=1_000_000).contains(&self.floor_ppm)
            || self.peak_ratio_ppm > 1_000_000
            || self.min_open_us > 1_000_000
            || self.min_close_us > 1_000_000
        {
            return Err(ValidationError::new(
                "口パク解析の設定値が範囲外です".into(),
            ));
        }
        Ok(())
    }
}
impl LipSyncAnalysis {
    /// Structural validation and source binding, not proof of actual PCM analysis.
    pub fn validate(&self, probe: &AudioProbe) -> Result<(), ValidationError> {
        probe.validate()?;
        self.settings.validate()?;
        let invalid = || ValidationError::new("口パクの原本・解析版・区間が不正です".into());
        if self.version != LIPSYNC_ANALYSIS_VERSION
            || self.source_sha256 != probe.sha256
            || self.sample_rate != probe.sample_rate
            || self.source_frames != probe.frames
            || self.sample_rate == 0
            || self.source_frames == 0
            || self.open.len() > MAX_LIPSYNC_WINDOWS as usize
            || !self.peak_rms.is_finite()
            || self.peak_rms < 0.
            || !self.threshold_rms.is_finite()
            || self.threshold_rms <= 0.
        {
            return Err(invalid());
        }
        let window_count = (u128::from(self.source_frames) * 1_000_000)
            .div_ceil(u128::from(self.sample_rate) * u128::from(self.settings.window_us));
        if window_count > u128::from(MAX_LIPSYNC_WINDOWS) {
            return Err(invalid());
        }
        let expected_threshold = (f64::from(self.settings.floor_ppm) / 1_000_000.)
            .max(self.peak_rms * f64::from(self.settings.peak_ratio_ppm) / 1_000_000.);
        if !expected_threshold.is_finite()
            || (self.threshold_rms - expected_threshold).abs() > expected_threshold * 1e-12
        {
            return Err(invalid());
        }
        let mut previous_end = None;
        for span in &self.open {
            if span.start_frame >= span.end_frame
                || span.end_frame > self.source_frames
                || previous_end.is_some_and(|end| end >= span.start_frame)
                || self.peak_rms < self.threshold_rms
                || u128::from(span.end_frame - span.start_frame) * 1_000_000
                    < u128::from(self.sample_rate) * u128::from(self.settings.min_open_us)
            {
                return Err(invalid());
            }
            previous_end = Some(span.end_frame);
        }
        Ok(())
    }
}
