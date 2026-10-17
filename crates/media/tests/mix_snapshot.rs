#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{
    fs, os::unix::fs::PermissionsExt, path::PathBuf, sync::atomic::AtomicBool, time::Duration,
};
use sungeki_media::{LoudnessError, measure_mix_loudness};

struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
#[ignore = "requires SSD VVM_MEDIA_TEST_ROOT and real VVM_MEDIA_FFMPEG"]
fn source_change_after_capture_cannot_change_the_measured_signal() {
    let base = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap());
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_MEDIA_FFMPEG").unwrap());
    let root = Root(base.join(format!("mix-snapshot-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&root.0).unwrap();
    let work = root.0.join("work");
    fs::create_dir(&work).unwrap();
    let source = root.0.join("input.wav");
    let mut wav = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for i in 0..48000 {
        let value = 0.1 * (std::f32::consts::TAU * (i % 48) as f32 / 48.).sin();
        wav.write_sample(value).unwrap();
        wav.write_sample(value).unwrap();
    }
    wav.finalize().unwrap();
    fs::copy(&source, root.0.join("expected.wav")).unwrap();
    let hash = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
    let flag = AtomicBool::new(false);
    let reference = measure_mix_loudness(
        &source,
        &hash,
        &ffmpeg,
        &work,
        Duration::from_secs(10),
        &flag,
    )
    .unwrap();
    let worker = root.0.join("mutate.sh");
    let quoted = format!("'{}'", ffmpeg.to_str().unwrap().replace('\'', "'\\''"));
    fs::write(&worker, format!("#!/bin/sh\nset -e\nprintf changed > ../../input.wav\ncmp source.wav ../../expected.wav\nexec {quoted} \"$@\"\n")).unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    let result = measure_mix_loudness(
        &source,
        &hash,
        &worker,
        &work,
        Duration::from_secs(10),
        &flag,
    )
    .unwrap();
    assert_eq!(result, reference);
    assert_eq!(fs::read(&source).unwrap(), b"changed");
    assert!(matches!(
        measure_mix_loudness(
            &source,
            &hash,
            &ffmpeg,
            &work,
            Duration::from_secs(10),
            &flag
        ),
        Err(LoudnessError::InvalidReport)
    ));
    assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
}
