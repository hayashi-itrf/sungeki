//! A constant source-relative speech gain estimate, never a limiter or master mix.
use crate::{LOUDNESS_VERSION, LoudnessAnalysis, LoudnessValue};
use serde::{Deserialize, Serialize};
use sungeki_project::{MAX_SPEECH_GAIN_DB, MIN_SPEECH_GAIN_DB};

/// Extra headroom for the 0.1 dB summary precision. Not a codec-overshoot guarantee.
pub const GAIN_PEAK_MARGIN_DB: f64 = 0.1;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GainProposal {
    pub target_lufs: f64,
    pub max_true_peak_db: f64,
    pub requested_gain_db: f64,
    // Replace the existing speech gain; do not add this to it.
    pub gain_db: f64,
    pub predicted_lufs: f64,
    pub predicted_true_peak_db: f64,
    pub peak_margin_db: f64,
    pub limited_by_peak: bool,
    pub limited_by_gain_range: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainProposalError {
    InvalidTarget,
    InvalidMeasurement,
    NotMeasurable,
    PeakLimitUnreachable,
}
impl std::fmt::Display for GainProposalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidTarget => "目標音量またはピーク上限が範囲外です",
            Self::InvalidMeasurement => "対応する元音声の測定結果が必要です",
            Self::NotMeasurable => "測定不能な音声から音量調整値を計算できません",
            Self::PeakLimitUnreachable => "発話音量の調整範囲ではピーク上限を満たせません",
        })
    }
}
impl std::error::Error for GainProposalError {}
/// The host must first verify job/session/revision/asset freshness. This pure
/// function cannot check external files and does not mutate a Project.
/// Predictions require remeasurement after rendering (gating, mixing and codecs).
pub fn propose_speech_gain(
    source: &LoudnessAnalysis,
    target_lufs: f64,
    max_true_peak_db: f64,
) -> Result<GainProposal, GainProposalError> {
    if !target_lufs.is_finite()
        || !(-70.0..=0.).contains(&target_lufs)
        || target_lufs == -70.
        || !max_true_peak_db.is_finite()
        || !(-60.0..=0.).contains(&max_true_peak_db)
    {
        return Err(GainProposalError::InvalidTarget);
    }
    if source.version != LOUDNESS_VERSION
        || source.method != "ffmpeg_ebur128_summary_192k"
        || !source.source_pre_gain
        || source.dual_mono
    {
        return Err(GainProposalError::InvalidMeasurement);
    }
    let LoudnessValue::Measured {
        integrated_lufs,
        true_peak_db,
        range_lu,
        threshold_lufs,
    } = source.measurement
    else {
        return Err(GainProposalError::NotMeasurable);
    };
    if !integrated_lufs.is_finite()
        || integrated_lufs <= -70.
        || !true_peak_db.is_finite()
        || !range_lu.is_finite()
        || range_lu < 0.
        || !threshold_lufs.is_finite()
    {
        return Err(GainProposalError::InvalidMeasurement);
    }
    let requested_gain_db = target_lufs - integrated_lufs;
    let bounded = requested_gain_db.clamp(MIN_SPEECH_GAIN_DB, MAX_SPEECH_GAIN_DB);
    let peak_cap = max_true_peak_db - GAIN_PEAK_MARGIN_DB - true_peak_db;
    // Never silently clamp up to the gain floor and violate the peak constraint.
    if peak_cap < MIN_SPEECH_GAIN_DB {
        return Err(GainProposalError::PeakLimitUnreachable);
    }
    let gain_db = bounded.min(peak_cap);
    Ok(GainProposal {
        target_lufs,
        max_true_peak_db,
        requested_gain_db,
        gain_db,
        predicted_lufs: integrated_lufs + gain_db,
        predicted_true_peak_db: true_peak_db + gain_db,
        peak_margin_db: GAIN_PEAK_MARGIN_DB,
        limited_by_peak: peak_cap < bounded,
        limited_by_gain_range: bounded != requested_gain_db,
    })
}
