#![cfg(unix)]
use hound::{SampleFormat, WavSpec, WavWriter};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::*;
fn wav(seconds: f64, amplitude: f32, channels: u16, inverse: bool, hz: f64) -> VerifiedWav {
    let mut data = Cursor::new(vec![]);
    {
        let mut writer = WavWriter::new(
            &mut data,
            WavSpec {
                sample_rate: 48000,
                channels,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap();
        for i in 0..(seconds * 48000.) as usize {
            let sample = amplitude
                * (std::f64::consts::TAU * hz * i as f64 / 48000. + std::f64::consts::FRAC_PI_4)
                    .sin() as f32;
            for channel in 0..channels {
                writer
                    .write_sample(if inverse && channel == 1 {
                        -sample
                    } else {
                        sample
                    })
                    .unwrap();
            }
        }
        writer.finalize().unwrap();
    }
    verify_wav(
        data.into_inner(),
        ExpectedAudio {
            sample_rate: 48000,
            channels,
        },
    )
    .unwrap()
}
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn root(base: &Path) -> Root {
    let path = base.join(format!("loudness-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path).unwrap();
    Root(path)
}
#[test]
fn short_cancelled_and_failed_worker_never_leave_captures() {
    let root = root(&std::env::temp_dir());
    let short = wav(0.1, 0.1, 1, false, 1000.);
    assert!(matches!(
        measure_loudness(
            &short,
            Path::new("/not/launched"),
            &root.0,
            Duration::from_secs(1),
            &AtomicBool::new(false)
        )
        .unwrap()
        .measurement,
        LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::TooShort,
            true_peak_db: None
        }
    ));
    assert!(matches!(
        measure_loudness(
            &short,
            Path::new("/not/launched"),
            &root.0,
            Duration::from_secs(1),
            &AtomicBool::new(true)
        ),
        Err(LoudnessError::Cancelled)
    ));
    let audio = wav(1., 0.1, 1, false, 1000.);
    let bytes = audio.bytes().to_vec();
    assert!(matches!(
        measure_loudness(
            &audio,
            Path::new("/usr/bin/false"),
            &root.0,
            Duration::from_secs(1),
            &AtomicBool::new(false)
        ),
        Err(LoudnessError::Worker(_))
    ));
    assert!(
        measure_loudness(
            &audio,
            Path::new("relative"),
            &root.0,
            Duration::from_secs(1),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(matches!(
        measure_loudness(
            &audio,
            Path::new("/bin/echo"),
            &root.0,
            Duration::from_secs(1),
            &AtomicBool::new(false)
        ),
        Err(LoudnessError::InvalidReport)
    ));
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    assert_eq!(audio.bytes(), bytes);
}
#[test]
fn deadline_and_cancellation_settle_worker_before_capture_cleanup() {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{Arc, atomic::Ordering},
        thread,
    };
    let root = root(&std::env::temp_dir());
    let worker = root.0.join("wait.sh");
    fs::write(&worker, "#!/bin/sh\nsleep 30\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let audio = wav(1., 0.1, 1, false, 1000.);
    assert!(matches!(
        measure_loudness(
            &audio,
            &worker,
            &root.0,
            Duration::from_millis(30),
            &AtomicBool::new(false)
        ),
        Err(LoudnessError::Stopped(StopReason::Deadline))
    ));
    let flag = Arc::new(AtomicBool::new(false));
    let trigger = flag.clone();
    let thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(60));
        trigger.store(true, Ordering::Release);
    });
    assert!(matches!(
        measure_loudness(&audio, &worker, &root.0, Duration::from_secs(2), &flag),
        Err(LoudnessError::Cancelled)
    ));
    thread.join().unwrap();
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
}
fn values(result: LoudnessAnalysis) -> (f64, f64) {
    let LoudnessValue::Measured {
        integrated_lufs,
        true_peak_db,
        ..
    } = result.measurement
    else {
        panic!("not measurable")
    };
    (integrated_lufs, true_peak_db)
}
#[test]
#[ignore = "requires configured SSD test root and FFmpeg with ebur128 true peak"]
fn real_loudness_tone_scaling_stereo_silence_and_intersample_peak() {
    let base = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").expect("VVM_MEDIA_TEST_ROOT"));
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_MEDIA_FFMPEG").expect("VVM_MEDIA_FFMPEG"));
    let root = root(&base);
    let flag = AtomicBool::new(false);
    let measure = |audio: &VerifiedWav| {
        let bytes = audio.bytes().to_vec();
        let result =
            measure_loudness(audio, &ffmpeg, &root.0, Duration::from_secs(10), &flag).unwrap();
        assert_eq!(result.source_sha256, audio.info().content_hash);
        assert!(result.source_pre_gain);
        assert!(!result.dual_mono);
        assert_eq!(audio.bytes(), bytes);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        result
    };
    let mono = values(measure(&wav(4., 0.1, 1, false, 1000.)));
    // Exercise incomplete 100 ms blocks and the old dynamic path's 3 s boundary.
    // A steady 1 kHz tone keeps calibrated level independent of source duration.
    for seconds in [0.4, 0.401, 0.799, 1., 2.999, 3., 3.001, 4.099] {
        let audio = wav(seconds, 0.1, 1, false, 1000.);
        let report = measure(&audio);
        assert_eq!(report.source_frames, audio.info().frames);
        let measured = values(report);
        assert!((measured.0 + 23.).abs() < 0.2, "{seconds}: {measured:?}");
        assert!((measured.1 + 20.).abs() < 0.1, "{seconds}: {measured:?}");
    }
    let half = values(measure(&wav(4., 0.05, 1, false, 1000.)));
    let stereo = values(measure(&wav(4., 0.1, 2, false, 1000.)));
    let opposite = values(measure(&wav(4., 0.1, 2, true, 1000.)));
    assert!((mono.0 + 23.0).abs() < 0.2, "{mono:?}");
    assert!((mono.1 + 20.).abs() < 0.1);
    // Each summary value is rounded to 0.1; subtracting two can add 0.1 error.
    println!("mono={mono:?} half={half:?} stereo={stereo:?}");
    assert!((half.0 - mono.0 + 6.0206).abs() <= 0.10001);
    assert!((half.1 - mono.1 + 6.0206).abs() <= 0.10001);
    assert!((stereo.0 - mono.0 - 3.0103).abs() <= 0.10001);
    assert_eq!(stereo, opposite);
    assert!(matches!(
        measure(&wav(1., 0., 1, false, 1000.)).measurement,
        LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::BelowGateOrSilence,
            true_peak_db: None
        }
    ));
    assert!(matches!(
        measure(&wav(1., 0.00001, 1, false, 1000.)).measurement,
        LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::BelowGateOrSilence,
            true_peak_db: Some(_)
        }
    ));
    assert!(values(measure(&wav(1., 1.2, 1, false, 1000.))).1 > 0.0);
    let high = wav(4., 0.8, 1, false, 12000.);
    let peak = analyze_waveform(&high, 256, &flag).unwrap().sample_peak[0];
    let measured = values(measure(&high));
    assert!(
        measured.1 > 20. * peak.log10() + 0.5,
        "true={measured:?} sample={peak}"
    );
    println!("mono={mono:?} half={half:?} stereo={stereo:?} true_peak_high={measured:?}");
}
