#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::{Mp3DecodeError, StopReason, decode_mp3, decode_mp3_file};
struct Root(PathBuf);
impl Root {
    fn new(base: &Path) -> Self {
        let p = base.join(format!("mp3-file-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn worker(&self, script: &str) -> PathBuf {
        let p = self.0.join("worker");
        fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        p
    }
    fn no_work(&self) {
        assert!(!fs::read_dir(&self.0).unwrap().any(|p| {
            p.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("mp3-file-")
        }));
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn bounded_file_decode_cleans_up_failures_and_success() {
    let root = Root::new(&std::env::temp_dir());
    let flag = AtomicBool::new(false);
    let worker = root.worker("head -c 80 /dev/zero");
    let file = decode_mp3_file(
        b"fixture",
        &worker,
        &root.0,
        Duration::from_secs(3),
        10,
        &flag,
    )
    .unwrap();
    assert_eq!(file.info().frames, 10);
    assert_eq!(fs::metadata(file.path()).unwrap().len(), 124);
    let measured = sungeki_media::measure_decoded_mp3_loudness(
        &file,
        &worker,
        &root.0,
        Duration::from_secs(3),
        &flag,
    )
    .unwrap();
    assert!(measured.source_pre_gain);
    assert!(matches!(
        measured.measurement,
        sungeki_media::LoudnessValue::NotMeasurable { .. }
    ));
    let waveform = sungeki_media::analyze_decoded_mp3_waveform(&file, 3, &flag).unwrap();
    assert_eq!(waveform.source_frames, 10);
    assert_eq!(waveform.window(0, 0, 0, 4).unwrap().end_frame, 10);
    let mut changed = fs::read(file.path()).unwrap();
    changed[44] = 1;
    fs::write(file.path(), changed).unwrap();
    assert!(matches!(
        sungeki_media::measure_decoded_mp3_loudness(
            &file,
            &worker,
            &root.0,
            Duration::from_secs(3),
            &flag,
        ),
        Err(sungeki_media::LoudnessError::InvalidReport)
    ));
    assert!(sungeki_media::analyze_decoded_mp3_waveform(&file, 3, &flag).is_err());
    drop(file);
    root.no_work();
    let worker = root.worker("head -c 88 /dev/zero");
    assert!(matches!(
        decode_mp3_file(
            b"fixture",
            &worker,
            &root.0,
            Duration::from_secs(3),
            10,
            &flag
        ),
        Err(Mp3DecodeError::Stopped(StopReason::OutputLimit))
    ));
    root.no_work();
    let worker = root.worker("printf '\\000\\000\\200\\177\\000\\000\\000\\000'");
    assert!(matches!(
        decode_mp3_file(
            b"fixture",
            &worker,
            &root.0,
            Duration::from_secs(3),
            10,
            &flag
        ),
        Err(Mp3DecodeError::Wav(_))
    ));
    root.no_work();
    let worker = root.worker("sleep 5");
    assert!(matches!(
        decode_mp3_file(
            b"fixture",
            &worker,
            &root.0,
            Duration::from_millis(30),
            10,
            &flag
        ),
        Err(Mp3DecodeError::Stopped(StopReason::Deadline))
    ));
    root.no_work();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(30));
            flag.store(true, std::sync::atomic::Ordering::Release);
        });
        assert!(matches!(
            decode_mp3_file(
                b"fixture",
                &worker,
                &root.0,
                Duration::from_secs(3),
                10,
                &flag
            ),
            Err(Mp3DecodeError::Cancelled)
        ));
    });
    root.no_work();
    flag.store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        decode_mp3_file(
            b"fixture",
            &worker,
            &root.0,
            Duration::from_secs(3),
            10,
            &flag
        ),
        Err(Mp3DecodeError::Cancelled)
    ));
    root.no_work();
}
#[test]
#[ignore = "requires real FFmpeg and external SSD work root"]
fn real_long_mp3_and_canonical_short_decode() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let root = Root::new(&PathBuf::from(
        std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap(),
    ));
    let flag = AtomicBool::new(false);
    for seconds in [3, 180] {
        let path = root.0.join(format!("{seconds}.mp3"));
        assert!(
            std::process::Command::new(&ffmpeg)
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency=440:sample_rate=48000:duration={seconds}"),
                    "-c:a",
                    "libmp3lame",
                    "-q:a",
                    "2"
                ])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let bytes = fs::read(&path).unwrap();
        let file = decode_mp3_file(
            &bytes,
            &ffmpeg,
            &root.0,
            Duration::from_secs(60),
            48000 * 180,
            &flag,
        )
        .unwrap();
        assert_eq!(file.info().frames, seconds * 48000);
        assert_eq!(
            fs::metadata(file.path()).unwrap().len(),
            seconds * 48000 * 8 + 44
        );
        let measured = sungeki_media::measure_decoded_mp3_loudness(
            &file,
            &ffmpeg,
            &root.0,
            Duration::from_secs(60),
            &flag,
        )
        .unwrap();
        assert!(measured.source_pre_gain);
        assert_eq!(measured.source_frames, seconds * 48000);
        assert_eq!(measured.source_sha256, file.info().content_hash);
        let waveform = sungeki_media::analyze_decoded_mp3_waveform(&file, 256, &flag).unwrap();
        assert_eq!(waveform.source_frames, seconds * 48000);
        assert_eq!(waveform.source_sha256, file.info().content_hash);
        if seconds == 3 {
            let old = decode_mp3(&bytes, &ffmpeg, &root.0, Duration::from_secs(30), &flag).unwrap();
            assert_eq!(old.bytes(), fs::read(file.path()).unwrap());
            assert_eq!(old.info(), file.info());
            let old_measurement = sungeki_media::measure_loudness(
                &old,
                &ffmpeg,
                &root.0,
                Duration::from_secs(30),
                &flag,
            )
            .unwrap();
            assert_eq!(measured, old_measurement);
            let old_waveform = sungeki_media::analyze_waveform(&old, 256, &flag).unwrap();
            assert_eq!(waveform.rms, old_waveform.rms);
            assert_eq!(waveform.sample_peak, old_waveform.sample_peak);
            assert_eq!(waveform.levels().len(), old_waveform.levels().len());
            for (level, description) in waveform.levels().iter().enumerate() {
                for channel in 0..2 {
                    for start in (0..description.bucket_count()).step_by(4096) {
                        let actual = waveform.window(level, channel, start, 4096).unwrap();
                        let expected = old_waveform.window(level, channel, start, 4096).unwrap();
                        assert_eq!(actual.peaks, expected.peaks);
                        assert_eq!(actual.end_frame, expected.end_frame);
                    }
                }
            }
        } else {
            assert!(fs::metadata(file.path()).unwrap().len() > 64 * 1024 * 1024);
            assert!(matches!(
                sungeki_media::analyze_decoded_mp3_waveform(&file, 1, &flag),
                Err(sungeki_media::WaveformError::TooManyBuckets)
            ));
        }
        assert_eq!(fs::read(path).unwrap(), bytes);
        drop(file);
        root.no_work();
    }
}
