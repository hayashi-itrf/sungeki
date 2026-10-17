//! Constant gain preserves speech/BGM balance; never silently enable a limiter.
use crate::{RenderError, plan::error};
use sungeki_media::{LOUDNESS_VERSION, LoudnessAnalysis, LoudnessValue};
use sungeki_project::Normalization;

pub(crate) const LUFS_TOLERANCE: f64 = 0.2;
pub(crate) const PEAK_MARGIN: f64 = 0.1;

pub(crate) fn levels(report: &LoudnessAnalysis) -> Result<(f64, f64), RenderError> {
    if report.version != LOUDNESS_VERSION
        || report.source_pre_gain
        || report.dual_mono
        || report.method != "ffmpeg_ebur128_summary_192k"
        || report.sample_rate != 48000
        || report.channels != 2
    {
        return Err(error("全体音量の調整には固定ミックスの測定結果が必要です"));
    }
    if let LoudnessValue::Measured {
        integrated_lufs,
        true_peak_db,
        ..
    } = report.measurement
    {
        if integrated_lufs.is_finite() && integrated_lufs > -70. && true_peak_db.is_finite() {
            return Ok((integrated_lufs, true_peak_db));
        }
    }
    Err(error(
        "無音・短尺などで音量を測定できません。全体音量の調整を解除するか音声を確認してください",
    ))
}
pub(crate) fn gain(report: &LoudnessAnalysis, target: &Normalization) -> Result<f64, RenderError> {
    if !target.target_lufs.is_finite()
        || !target.true_peak_db.is_finite()
        || !(-70.0..=0.).contains(&target.target_lufs)
        || target.target_lufs == -70.
        || !(-20.0..=0.).contains(&target.true_peak_db)
    {
        return Err(error(
            "目標音量は-70 LUFSより大きく0以下、ピーク上限は-20〜0 dBTPで指定してください",
        ));
    }
    let (lufs, peak) = levels(report)?;
    let gain = target.target_lufs - lufs;
    if !(-60.0..=24.).contains(&gain) || peak + gain > target.true_peak_db - PEAK_MARGIN + 1e-9 {
        return Err(error(
            "音量バランスを保つ一定の増減では、目標音量とピーク上限を両立できません。目標音量を下げるか素材の音量を見直してください（リミッターは未対応）",
        ));
    }
    Ok(gain)
}
pub(crate) fn verify(report: &LoudnessAnalysis, target: &Normalization) -> Result<(), RenderError> {
    let (lufs, peak) = levels(report)?;
    if (lufs - target.target_lufs).abs() > LUFS_TOLERANCE + 1e-9
        || peak > target.true_peak_db - PEAK_MARGIN + 1e-9
    {
        return Err(error(format!(
            "出力音量が目標を満たしません（実測 {lufs:.1} LUFS / {peak:.1} dBTP）。目標音量を下げるかピーク上限を見直してください"
        )));
    }
    Ok(())
}
