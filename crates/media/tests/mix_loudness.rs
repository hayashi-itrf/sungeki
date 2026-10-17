#![cfg(unix)]
use hound::{SampleFormat, WavSpec, WavWriter};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::*;

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    work: PathBuf,
}
impl Fixture {
    fn new(base: &Path) -> Self {
        let root = base.join(format!("mix-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let work = root.join("work");
        fs::create_dir(&work).unwrap();
        Self {
            source: root.join("input.wav"),
            root,
            work,
        }
    }
    fn wav(&self, frames: u64, channels: u16, amplitude: f32) {
        let mut writer = WavWriter::create(
            &self.source,
            WavSpec {
                sample_rate: 48000,
                channels,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .unwrap();
        for i in 0..frames {
            let sample = amplitude * (std::f64::consts::TAU * (i % 48) as f64 / 48.).sin() as f32;
            for _ in 0..channels {
                writer.write_sample(sample).unwrap();
            }
        }
        writer.finalize().unwrap();
    }
    fn measure(&self, worker: &Path) -> Result<LoudnessAnalysis, LoudnessError> {
        let result = measure_mix_loudness(
            &self.source,
            &hash(&self.source),
            worker,
            &self.work,
            Duration::from_secs(60),
            &AtomicBool::new(false),
        );
        assert_eq!(fs::read_dir(&self.work).unwrap().count(), 0);
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn hash(path: &Path) -> String {
    let mut input = fs::File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = input.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    format!("{:x}", digest.finalize())
}

#[test]
fn short_mix_identity_cancellation_and_worker_errors_preserve_source() {
    let f = Fixture::new(&std::env::temp_dir());
    f.wav(19199, 2, 0.1);
    let original = hash(&f.source);
    let report = f.measure(Path::new("/not/launched")).unwrap();
    assert!(!report.source_pre_gain);
    assert_eq!(report.source_frames, 19199);
    assert_eq!(report.source_sha256, original);
    assert!(matches!(
        report.measurement,
        LoudnessValue::NotMeasurable {
            reason: LoudnessUnavailable::TooShort,
            ..
        }
    ));
    assert!(matches!(
        measure_mix_loudness(
            &f.source,
            &"0".repeat(64),
            Path::new("/not/launched"),
            &f.work,
            Duration::from_secs(1),
            &AtomicBool::new(false)
        ),
        Err(LoudnessError::InvalidReport)
    ));
    assert!(matches!(
        measure_mix_loudness(
            &f.source,
            &original,
            Path::new("/not/launched"),
            &f.work,
            Duration::from_secs(1),
            &AtomicBool::new(true)
        ),
        Err(LoudnessError::Cancelled)
    ));
    assert_eq!(hash(&f.source), original);
    f.wav(48000, 2, 0.1);
    let original = hash(&f.source);
    assert!(matches!(
        f.measure(Path::new("/usr/bin/false")),
        Err(LoudnessError::Worker(_))
    ));
    assert!(matches!(
        f.measure(Path::new("/bin/echo")),
        Err(LoudnessError::InvalidReport)
    ));
    assert_eq!(hash(&f.source), original);
}

#[test]
fn malformed_mix_and_symlink_are_rejected_before_worker_start() {
    let f = Fixture::new(&std::env::temp_dir());
    for (frames, channels, amplitude) in [(100, 1, 0.1), (100, 2, f32::NAN), (0, 2, 0.)] {
        f.wav(frames, channels, amplitude);
        assert!(matches!(
            f.measure(Path::new("/not/launched")),
            Err(LoudnessError::InvalidReport)
        ));
    }
    f.wav(100, 2, 0.1);
    let valid = fs::read(&f.source).unwrap();
    let mut trailing = valid.clone();
    trailing.extend_from_slice(b"unexpected");
    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(b"data\x08\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");
    let size = (duplicate.len() - 8) as u32;
    duplicate[4..8].copy_from_slice(&size.to_le_bytes());
    for bytes in [valid[..valid.len() - 1].to_vec(), trailing, duplicate] {
        fs::write(&f.source, bytes).unwrap();
        assert!(matches!(
            f.measure(Path::new("/not/launched")),
            Err(LoudnessError::InvalidReport)
        ));
    }
    fs::write(&f.source, &valid).unwrap();
    let link = f.root.join("link.wav");
    std::os::unix::fs::symlink(&f.source, &link).unwrap();
    assert!(
        measure_mix_loudness(
            &link,
            &hash(&f.source),
            Path::new("/not/launched"),
            &f.work,
            Duration::from_secs(1),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert_eq!(fs::read(&f.source).unwrap(), valid);
    assert_eq!(fs::read_dir(&f.work).unwrap().count(), 0);
}

#[test]
fn running_mix_deadline_and_cancellation_remove_private_capture() {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{Arc, atomic::Ordering},
        thread,
    };
    let f = Fixture::new(&std::env::temp_dir());
    f.wav(48000, 2, 0.1);
    let worker = f.root.join("wait.sh");
    fs::write(&worker, "#!/bin/sh\ntouch ../ready\nsleep 30\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let original = hash(&f.source);
    assert!(matches!(
        measure_mix_loudness(
            &f.source,
            &original,
            &worker,
            &f.work,
            Duration::from_millis(100),
            &AtomicBool::new(false)
        ),
        Err(LoudnessError::Stopped(StopReason::Deadline))
    ));
    let ready = f.work.join("ready");
    if ready.exists() {
        fs::remove_file(&ready).unwrap();
    }
    let flag = Arc::new(AtomicBool::new(false));
    let trigger = flag.clone();
    let handle = thread::spawn(move || {
        let start = std::time::Instant::now();
        while !ready.exists() && start.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(5));
        }
        let started = ready.exists();
        trigger.store(true, Ordering::Release);
        started
    });
    assert!(matches!(
        measure_mix_loudness(
            &f.source,
            &original,
            &worker,
            &f.work,
            Duration::from_secs(5),
            &flag
        ),
        Err(LoudnessError::Cancelled)
    ));
    assert!(handle.join().unwrap());
    fs::remove_file(f.work.join("ready")).unwrap();
    assert_eq!(fs::read_dir(&f.work).unwrap().count(), 0);
    assert_eq!(hash(&f.source), original);
}

#[test]
#[ignore = "requires SSD VVM_MEDIA_TEST_ROOT and real VVM_MEDIA_FFMPEG"]
fn real_mix_matches_source_measurement_and_accepts_over_64mib() {
    let base = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap());
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_MEDIA_FFMPEG").unwrap());
    let f = Fixture::new(&base);
    for amplitude in [0., 0.1] {
        f.wav(48000, 2, amplitude);
        let original = hash(&f.source);
        let wav = verify_wav(
            fs::read(&f.source).unwrap(),
            ExpectedAudio {
                sample_rate: 48000,
                channels: 2,
            },
        )
        .unwrap();
        let reference = measure_loudness(
            &wav,
            &ffmpeg,
            &f.work,
            Duration::from_secs(10),
            &AtomicBool::new(false),
        )
        .unwrap();
        let actual = f.measure(&ffmpeg).unwrap();
        assert_eq!(actual.measurement, reference.measurement);
        assert!(!actual.source_pre_gain);
        assert_eq!(hash(&f.source), original);
        println!("amplitude={amplitude}: {:?}", actual.measurement);
    }
    f.wav(180 * 48000, 2, 0.1);
    assert!(fs::metadata(&f.source).unwrap().len() > MAX_WAV_BYTES as u64);
    let original = hash(&f.source);
    let report = f.measure(&ffmpeg).unwrap();
    assert_eq!(report.source_frames, 8640000);
    let LoudnessValue::Measured {
        integrated_lufs,
        true_peak_db,
        ..
    } = report.measurement
    else {
        panic!("unmeasurable")
    };
    assert!((integrated_lufs + 20.).abs() < 0.2);
    assert!((true_peak_db + 20.).abs() < 0.1);
    assert_eq!(hash(&f.source), original);
    println!("180s mix: {integrated_lufs} LUFS / {true_peak_db} dBTP");
}
