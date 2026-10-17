use hound::{SampleFormat, WavSpec, WavWriter};
use std::{io::Cursor, sync::atomic::AtomicBool};
use sungeki_media::{
    ExpectedAudio, SamplePeak, VerifiedWav, WaveformError, analyze_waveform, verify_wav,
};
fn wav(channels: u16, bits: u16, float: bool, values: &[f64]) -> VerifiedWav {
    let mut bytes = Cursor::new(vec![]);
    {
        let mut w = WavWriter::new(
            &mut bytes,
            WavSpec {
                channels,
                sample_rate: 44100,
                bits_per_sample: bits,
                sample_format: if float {
                    SampleFormat::Float
                } else {
                    SampleFormat::Int
                },
            },
        )
        .unwrap();
        for &v in values {
            if float {
                w.write_sample(v as f32).unwrap();
            } else {
                w.write_sample((v * 2f64.powi(i32::from(bits) - 1)) as i32)
                    .unwrap();
            }
        }
        w.finalize().unwrap();
    }
    let mut bytes = bytes.into_inner();
    if bytes.len() % 2 != 0 {
        bytes.push(0);
        let len = bytes.len() as u32 - 8;
        bytes[4..8].copy_from_slice(&len.to_le_bytes());
    }
    verify_wav(
        bytes,
        ExpectedAudio {
            sample_rate: 44100,
            channels,
        },
    )
    .unwrap()
}
#[test]
fn every_resolution_matches_original_extrema_and_partial_tail() {
    let values = [0.25, 0.5, -0.75, -0.5, 0.125, 0.875, -0.125];
    let original = wav(1, 32, true, &values);
    let before = original.bytes().to_vec();
    for base in [1, 2, 3, 256] {
        let analysis = analyze_waveform(&original, base, &AtomicBool::new(false)).unwrap();
        assert_eq!(analysis.source_sha256, original.info().content_hash);
        assert_eq!(analysis.source_frames, 7);
        for (index, level) in analysis.levels().iter().enumerate() {
            let view = analysis.window(index, 0, 0, 4096).unwrap();
            assert_eq!(view.start_frame, 0);
            assert_eq!(view.end_frame, 7);
            for (actual, expected) in view
                .peaks
                .iter()
                .zip(values.chunks(level.frames_per_bucket as usize))
            {
                assert_eq!(
                    actual.min,
                    expected.iter().copied().fold(f64::INFINITY, f64::min)
                );
                assert_eq!(
                    actual.max,
                    expected.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                );
            }
            assert_eq!(
                view.peaks.len(),
                values.len().div_ceil(level.frames_per_bucket as usize)
            );
        }
        assert_eq!(analysis.levels().last().unwrap().bucket_count(), 1);
    }
    assert_eq!(original.bytes(), before);
}
#[test]
fn stereo_channels_do_not_cancel_each_other_or_gain_a_zero_baseline() {
    let original = wav(2, 16, false, &[0.5, -0.5, 0.25, -0.25, 0.75, -0.75]);
    let a = analyze_waveform(&original, 2, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        a.window(0, 0, 0, 1).unwrap().peaks,
        &[SamplePeak {
            min: 0.25,
            max: 0.5
        }]
    );
    assert_eq!(
        a.window(0, 1, 0, 1).unwrap().peaks,
        &[SamplePeak {
            min: -0.5,
            max: -0.25
        }]
    );
    assert_eq!(a.sample_peak, vec![0.75, 0.75]);
    let expected = ((0.25f64 + 0.0625 + 0.5625) / 3.).sqrt();
    assert_eq!(a.rms, vec![expected, expected]);
}
#[test]
fn pcm_formats_float_overrange_and_silence_are_measured_without_normalization() {
    for bits in [8, 16, 24, 32] {
        let original = wav(1, bits, false, &[-0.5, 0.25, 0.75, 0.]);
        let a = analyze_waveform(&original, 4, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            a.window(0, 0, 0, 1).unwrap().peaks,
            &[SamplePeak {
                min: -0.5,
                max: 0.75
            }]
        );
    }
    let a = analyze_waveform(&wav(1, 32, true, &[2., -2.]), 2, &AtomicBool::new(false)).unwrap();
    assert_eq!(a.rms, vec![2.]);
    assert_eq!(a.sample_peak, vec![2.]);
    let silence =
        analyze_waveform(&wav(1, 16, false, &[0.; 7]), 3, &AtomicBool::new(false)).unwrap();
    assert_eq!(silence.rms, vec![0.]);
    assert_eq!(silence.sample_peak, vec![0.]);
}
#[test]
fn window_limits_channels_and_exact_end_are_enforced() {
    let a = analyze_waveform(&wav(1, 16, false, &[0.; 7]), 2, &AtomicBool::new(false)).unwrap();
    let tail = a.window(0, 0, 3, 4096).unwrap();
    assert_eq!(
        (tail.start_frame, tail.end_frame, tail.peaks.len()),
        (6, 7, 1)
    );
    let empty = a.window(0, 0, 4, 1).unwrap();
    assert_eq!(
        (empty.start_frame, empty.end_frame, empty.peaks.len()),
        (7, 7, 0)
    );
    for (level, channel, start, count) in [
        (9, 0, 0, 1),
        (0, 1, 0, 1),
        (0, 0, 5, 1),
        (0, 0, 0, 0),
        (0, 0, 0, 4097),
        (0, 0, usize::MAX, 1),
    ] {
        assert!(matches!(
            a.window(level, channel, start, count),
            Err(WaveformError::Window)
        ));
    }
}
#[test]
fn bounded_analysis_rejects_invalid_settings_excessive_resolution_and_cancellation() {
    let original = wav(1, 16, false, &vec![0.; 262145]);
    for base in [0, 65537] {
        assert!(matches!(
            analyze_waveform(&original, base, &AtomicBool::new(false)),
            Err(WaveformError::Settings)
        ));
    }
    assert!(matches!(
        analyze_waveform(&original, 1, &AtomicBool::new(false)),
        Err(WaveformError::TooManyBuckets)
    ));
    assert!(matches!(
        analyze_waveform(&original, 256, &AtomicBool::new(true)),
        Err(WaveformError::Cancelled)
    ));
}
