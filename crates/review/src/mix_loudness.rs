use sungeki_media::{LoudnessAnalysis, LoudnessUnavailable, LoudnessValue};
/// A mix measurement is not listening approval, nor an AAC measurement.
pub fn describe_mix_loudness(
    analysis: &LoudnessAnalysis,
) -> (String, Option<(&'static str, String)>) {
    match &analysis.measurement {
        LoudnessValue::Measured {
            integrated_lufs,
            true_peak_db,
            range_lu,
            ..
        } => {
            let description = format!(
                "出力直前PCM: {:.1} LUFS、true peak {:.2} dBTP、音量幅 {:.1} LU。",
                integrated_lufs, true_peak_db, range_lu
            );
            let warning = (*true_peak_db > 0.).then(|| ("AUDIO_MIX_PEAK", format!("出力直前の混合音声のtrue peakが{true_peak_db:.2} dBTPで0を超えています。音割れの可能性があるため、音量設定と確認音声を確認してください。")));
            (description, warning)
        }
        LoudnessValue::NotMeasurable {
            reason,
            true_peak_db,
        } => {
            let reason = match reason {
                LoudnessUnavailable::TooShort => "測定に必要な長さがありません",
                LoudnessUnavailable::BelowGateOrSilence => "無音または測定の下限未満です",
            };
            let peak = true_peak_db
                .map(|p| format!("true peak {p:.2} dBTP。"))
                .unwrap_or_default();
            let message = format!("出力直前PCMの音量は測定不能: {reason}。{peak}");
            (message.clone(), Some(("AUDIO_MIX_NOT_MEASURABLE", message)))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn analysis(value: LoudnessValue) -> LoudnessAnalysis {
        LoudnessAnalysis {
            version: 2,
            method: "test".into(),
            source_sha256: "a".repeat(64),
            source_frames: 48000,
            sample_rate: 48000,
            channels: 2,
            source_pre_gain: false,
            dual_mono: false,
            measurement: value,
        }
    }
    #[test]
    fn reports_measured_pcm_and_warns_only_above_full_scale() {
        for (peak, warn) in [(-1., false), (0., false), (0.1, true)] {
            let (description, issue) = describe_mix_loudness(&analysis(LoudnessValue::Measured {
                integrated_lufs: -16.,
                true_peak_db: peak,
                range_lu: 3.,
                threshold_lufs: -26.,
            }));
            assert!(description.contains("出力直前PCM: -16.0 LUFS"));
            assert_eq!(issue.is_some(), warn);
        }
    }
    #[test]
    fn silence_and_short_audio_are_never_presented_as_zero_lufs() {
        for reason in [
            LoudnessUnavailable::TooShort,
            LoudnessUnavailable::BelowGateOrSilence,
        ] {
            let (description, warning) =
                describe_mix_loudness(&analysis(LoudnessValue::NotMeasurable {
                    reason,
                    true_peak_db: None,
                }));
            assert!(description.contains("測定不能"));
            assert!(!description.contains("0.0 LUFS"));
            assert_eq!(warning.unwrap().0, "AUDIO_MIX_NOT_MEASURABLE");
        }
    }
}
