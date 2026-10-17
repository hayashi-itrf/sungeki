#![cfg(unix)]
use sungeki_media::*;
fn source(lufs: f64, peak: f64) -> LoudnessAnalysis {
    LoudnessAnalysis {
        version: LOUDNESS_VERSION,
        method: "ffmpeg_ebur128_summary_192k".into(),
        source_sha256: "a".repeat(64),
        source_frames: 48000,
        sample_rate: 48000,
        channels: 1,
        source_pre_gain: true,
        dual_mono: false,
        measurement: LoudnessValue::Measured {
            integrated_lufs: lufs,
            true_peak_db: peak,
            range_lu: 1.,
            threshold_lufs: lufs - 10.,
        },
    }
}
#[test]
fn target_estimate_and_peak_limited_estimate_are_distinct_and_source_relative() {
    let original = source(-24.5, -6.);
    let before = original.clone();
    let target = propose_speech_gain(&original, -22., -1.).unwrap();
    assert_eq!(target.gain_db, 2.5);
    assert_eq!(target.predicted_lufs, -22.);
    assert!(!target.limited_by_peak && !target.limited_by_gain_range);
    let limited = propose_speech_gain(&original, -16., -1.).unwrap();
    assert!((limited.gain_db - 4.9).abs() < 1e-9);
    assert!((limited.predicted_lufs + 19.6).abs() < 1e-9);
    assert!((limited.predicted_true_peak_db + 1.1).abs() < 1e-9);
    assert!(limited.limited_by_peak);
    assert!(limited.predicted_lufs < limited.target_lufs);
    assert_eq!(original, before);
    assert_eq!(propose_speech_gain(&original, -16., -1.).unwrap(), limited);
}
#[test]
fn gain_bounds_never_override_peak_limit_and_attenuation_is_supported() {
    let upper = propose_speech_gain(&source(-60., -40.), -16., -1.).unwrap();
    assert_eq!(upper.gain_db, 24.);
    assert!(upper.limited_by_gain_range);
    let lower = propose_speech_gain(&source(10., 10.), -60., -1.).unwrap();
    assert_eq!(lower.gain_db, -60.);
    assert!(lower.limited_by_gain_range);
    assert_eq!(
        propose_speech_gain(&source(60., 60.), -16., -1.),
        Err(GainProposalError::PeakLimitUnreachable)
    );
    let down = propose_speech_gain(&source(-10., -2.), -20., -1.).unwrap();
    assert_eq!(down.gain_db, -10.);
    assert_eq!(down.predicted_true_peak_db, -12.);
}
#[test]
fn unsupported_invalid_and_unmeasurable_inputs_never_produce_a_gain() {
    let good = source(-24., -6.);
    for target in [f64::NAN, f64::INFINITY, -70., -71., 1.] {
        assert_eq!(
            propose_speech_gain(&good, target, -1.),
            Err(GainProposalError::InvalidTarget)
        );
    }
    for peak in [f64::NAN, f64::NEG_INFINITY, -61., 1.] {
        assert_eq!(
            propose_speech_gain(&good, -16., peak),
            Err(GainProposalError::InvalidTarget)
        );
    }
    let mut bad = good.clone();
    bad.version = 1;
    assert_eq!(
        propose_speech_gain(&bad, -16., -1.),
        Err(GainProposalError::InvalidMeasurement)
    );
    bad = good.clone();
    bad.source_pre_gain = false;
    assert_eq!(
        propose_speech_gain(&bad, -16., -1.),
        Err(GainProposalError::InvalidMeasurement)
    );
    for reason in [
        LoudnessUnavailable::TooShort,
        LoudnessUnavailable::BelowGateOrSilence,
    ] {
        bad = good.clone();
        bad.measurement = LoudnessValue::NotMeasurable {
            reason,
            true_peak_db: None,
        };
        assert_eq!(
            propose_speech_gain(&bad, -16., -1.),
            Err(GainProposalError::NotMeasurable)
        );
    }
    assert_eq!(
        propose_speech_gain(&source(-70., -20.), -16., -1.),
        Err(GainProposalError::InvalidMeasurement)
    );
    assert_eq!(
        propose_speech_gain(&source(-24., f64::NAN), -16., -1.),
        Err(GainProposalError::InvalidMeasurement)
    );
}
