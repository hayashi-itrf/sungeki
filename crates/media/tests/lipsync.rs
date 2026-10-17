use hound::{SampleFormat, WavSpec, WavWriter};
use std::{io::Cursor, sync::atomic::AtomicBool};
use sungeki_media::{
    ExpectedAudio, LipSyncError, LipSyncSettings, MouthInterval, analyze_lipsync, verify_wav,
};

fn wav(
    rate: u32,
    channels: u16,
    bits: u16,
    float: bool,
    samples: &[f64],
) -> sungeki_media::VerifiedWav {
    let mut bytes = Cursor::new(vec![]);
    {
        let mut writer = WavWriter::new(
            &mut bytes,
            WavSpec {
                channels,
                sample_rate: rate,
                bits_per_sample: bits,
                sample_format: if float {
                    SampleFormat::Float
                } else {
                    SampleFormat::Int
                },
            },
        )
        .unwrap();
        for &v in samples {
            if float {
                writer.write_sample(v as f32).unwrap();
            } else {
                writer
                    .write_sample((v * 2f64.powi(i32::from(bits) - 1)) as i32)
                    .unwrap();
            }
        }
        writer.finalize().unwrap();
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
            sample_rate: rate,
            channels,
        },
    )
    .unwrap()
}
fn span(start_frame: u64, end_frame: u64) -> MouthInterval {
    MouthInterval {
        start_frame,
        end_frame,
    }
}
fn analyze(wav: &sungeki_media::VerifiedWav) -> sungeki_media::LipSyncAnalysis {
    analyze_lipsync(wav, LipSyncSettings::default(), &AtomicBool::new(false)).unwrap()
}
#[test]
fn silence_and_low_noise_never_open_even_after_peak_scaling() {
    for amplitude in [0., 0.001, -0.001] {
        let a = analyze(&wav(24000, 1, 16, false, &vec![amplitude; 24000]));
        assert!(a.open.is_empty());
        assert!(!a.is_open_at(0));
        assert!(!a.is_open_at(u64::MAX));
    }
}
#[test]
fn uses_right_channel_and_does_not_cancel_opposite_phase_stereo() {
    for pair in [[0., 0.5], [0.5, -0.5]] {
        let samples: Vec<_> = (0..1000).flat_map(|_| pair).collect();
        let a = analyze(&wav(1000, 2, 16, false, &samples));
        assert_eq!(a.open, vec![span(0, 1000)]);
        assert!((a.peak_rms - ((pair[0] * pair[0] + pair[1] * pair[1]) / 2.).sqrt()).abs() < 1e-10);
        assert!(a.is_open_at(999999));
        assert!(!a.is_open_at(1000000));
    }
}
#[test]
fn filters_bursts_and_internal_gaps_without_extending_edge_silence() {
    let mut samples = vec![0.; 600];
    samples[20..40].fill(0.5); // 20ms isolated click, removed
    samples[100..200].fill(0.5);
    samples[240..340].fill(0.5); // 40ms inner gap, filled
    samples[420..520].fill(0.5); // exactly 80ms gap, preserved
    let a = analyze(&wav(1000, 1, 16, false, &samples));
    assert_eq!(a.open, vec![span(100, 340), span(420, 520)]);
    for (us, open) in [
        (99999, false),
        (100000, true),
        (339999, true),
        (340000, false),
        (420000, true),
        (520000, false),
    ] {
        assert_eq!(a.is_open_at(us), open, "{us}");
    }
}
#[test]
fn supports_integer_depths_float_and_ancillary_padding_identically() {
    for (bits, float) in [
        (8, false),
        (16, false),
        (24, false),
        (32, false),
        (32, true),
    ] {
        let original = wav(44100, 1, bits, float, &vec![0.5; 4411]);
        let mut bytes = original.bytes().to_vec();
        bytes.splice(12..12, *b"JUNK\x01\0\0\0x\0");
        let len = bytes.len() as u32 - 8;
        bytes[4..8].copy_from_slice(&len.to_le_bytes());
        let padded = verify_wav(
            bytes,
            ExpectedAudio {
                sample_rate: 44100,
                channels: 1,
            },
        )
        .unwrap();
        let a = analyze(&padded);
        assert_eq!(a.open, vec![span(0, 4411)]);
        assert_eq!(a.peak_rms, 0.5);
        assert_eq!(a.source_sha256, padded.info().content_hash);
        assert_ne!(a.source_sha256, original.info().content_hash);
    }
}
#[test]
fn fractional_window_boundaries_and_final_partial_window_do_not_drift() {
    // 20ms * 10001Hz = 200.02 frames. The second window ends at 401,
    // the tenth at 2001. Integer truncation per window would be incorrect.
    let mut samples = vec![0.; 2100];
    samples[401..2001].fill(0.5);
    let a = analyze(&wav(10001, 1, 16, false, &samples));
    assert_eq!(a.open, vec![span(401, 2001)]);
    assert!(!a.is_open_at(40095));
    assert!(a.is_open_at(40096));
    assert!(!a.is_open_at(200080));
    let tiny = wav(44100, 1, 16, false, &[0.5]);
    let settings = LipSyncSettings {
        min_open_us: 0,
        ..Default::default()
    };
    let a = analyze_lipsync(&tiny, settings, &AtomicBool::new(false)).unwrap();
    assert_eq!(a.open, vec![span(0, 1)]);
    assert!(a.is_open_at(22));
    assert!(!a.is_open_at(23));
}
#[test]
fn validates_settings_cancels_and_bounds_output_even_for_low_sample_rate() {
    let source = wav(1, 1, 16, false, &[0.5]);
    for settings in [
        LipSyncSettings {
            window_us: 0,
            ..Default::default()
        },
        LipSyncSettings {
            floor_ppm: 0,
            ..Default::default()
        },
        LipSyncSettings {
            peak_ratio_ppm: 1_000_001,
            ..Default::default()
        },
        LipSyncSettings {
            min_close_us: 1_000_001,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            analyze_lipsync(&source, settings, &AtomicBool::new(false)),
            Err(LipSyncError::Settings)
        ));
    }
    assert!(matches!(
        analyze_lipsync(&source, Default::default(), &AtomicBool::new(true)),
        Err(LipSyncError::Cancelled)
    ));
    assert_eq!(analyze(&source).open, vec![span(0, 1)]);
    let too_long = wav(1, 1, 16, false, &vec![0.5; 2001]);
    assert!(matches!(
        analyze_lipsync(&too_long, Default::default(), &AtomicBool::new(false)),
        Err(LipSyncError::TooLong)
    ));
}
