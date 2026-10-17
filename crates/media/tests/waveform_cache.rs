#![cfg(unix)]
use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use sungeki_media::{ExpectedAudio, WaveformCache, WaveformCacheError, verify_wav};
use sungeki_project::{AudioProbe, TimeUs};
struct Fixture {
    root: PathBuf,
    path: PathBuf,
    bytes: Vec<u8>,
    probe: AudioProbe,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("vvm-wave-cache-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let mut bytes = Cursor::new(vec![]);
        {
            let mut w = hound::WavWriter::new(
                &mut bytes,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 24000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for i in 0..2048 {
                w.write_sample((i % 512) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let bytes = bytes.into_inner();
        let wav = verify_wav(
            bytes.clone(),
            ExpectedAudio {
                sample_rate: 24000,
                channels: 1,
            },
        )
        .unwrap();
        let i = wav.info();
        let probe = AudioProbe {
            encoding: None,
            sha256: i.content_hash.clone(),
            sample_rate: i.sample_rate,
            channels: i.channels,
            frames: i.frames,
            duration_us: TimeUs(i.duration_us),
            byte_length: bytes.len() as u64,
        };
        let path = root.join("source.wav");
        fs::write(&path, &bytes).unwrap();
        Self {
            root,
            path,
            bytes,
            probe,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn hits_share_analysis_but_revalidate_bytes_and_allow_another_original_path() {
    let f = Fixture::new();
    let mut c = WaveformCache::new(1024 * 1024, 4).unwrap();
    let flag = AtomicBool::new(false);
    let first = c.get(&f.path, &f.probe, 256, None, &flag).unwrap();
    assert!(!first.cache_hit);
    let same = c.get(&f.path, &f.probe, 256, None, &flag).unwrap();
    assert!(same.cache_hit);
    assert!(Arc::ptr_eq(&first.waveform, &same.waveform));
    let other = f.root.join("copy.wav");
    fs::write(&other, &f.bytes).unwrap();
    assert!(c.get(&other, &f.probe, 256, None, &flag).unwrap().cache_hit);
    let mut changed = f.bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    fs::write(&f.path, changed).unwrap();
    assert!(matches!(
        c.get(&f.path, &f.probe, 256, None, &flag),
        Err(WaveformCacheError::SourceChanged)
    ));
    fs::write(&f.path, &f.bytes).unwrap();
    assert!(
        c.get(&f.path, &f.probe, 256, None, &flag)
            .unwrap()
            .cache_hit
    );
    fs::remove_file(&f.path).unwrap();
    assert!(c.get(&f.path, &f.probe, 256, None, &flag).is_err());
    assert_eq!(first.waveform.source_sha256, f.probe.sha256);
}
#[test]
fn cached_hash_does_not_bypass_probe_validation_or_symlink_rejection() {
    let f = Fixture::new();
    let mut c = WaveformCache::new(1024 * 1024, 4).unwrap();
    let flag = AtomicBool::new(false);
    c.get(&f.path, &f.probe, 256, None, &flag).unwrap();
    let mut forged = f.probe.clone();
    forged.sample_rate = 48000;
    forged.duration_us = TimeUs((forged.frames * 1_000_000).div_ceil(48000));
    assert!(matches!(
        c.get(&f.path, &forged, 256, None, &flag),
        Err(WaveformCacheError::Wav(_))
    ));
    forged.frames += 1;
    assert!(matches!(
        c.get(&f.path, &forged, 256, None, &flag),
        Err(WaveformCacheError::InvalidProbe)
    ));
    let alias = f.root.join("alias.wav");
    std::os::unix::fs::symlink(&f.path, &alias).unwrap();
    assert!(c.get(&alias, &f.probe, 256, None, &flag).is_err());
    assert_eq!(c.len(), 1);
    assert_eq!(fs::read(&f.path).unwrap(), f.bytes);
}
#[test]
fn lru_and_resolution_keys_evict_without_invalidating_borrowers() {
    let f = Fixture::new();
    let mut c = WaveformCache::new(1024 * 1024, 2).unwrap();
    let flag = AtomicBool::new(false);
    let first = c.get(&f.path, &f.probe, 4, None, &flag).unwrap();
    c.get(&f.path, &f.probe, 8, None, &flag).unwrap();
    assert!(c.get(&f.path, &f.probe, 4, None, &flag).unwrap().cache_hit);
    c.get(&f.path, &f.probe, 16, None, &flag).unwrap();
    assert_eq!(c.len(), 2);
    assert!(!c.get(&f.path, &f.probe, 8, None, &flag).unwrap().cache_hit);
    assert!(c.retained_bytes() <= 1024 * 1024);
    c.clear();
    assert!(c.is_empty());
    assert_eq!(c.retained_bytes(), 0);
    assert_eq!(first.waveform.window(0, 0, 0, 1).unwrap().end_frame, 4);
}
#[test]
fn oversized_results_are_not_retained_and_cancelled_hits_do_not_succeed() {
    let f = Fixture::new();
    let flag = AtomicBool::new(false);
    let mut tiny = WaveformCache::new(1, 1).unwrap();
    for _ in 0..2 {
        assert!(
            !tiny
                .get(&f.path, &f.probe, 256, None, &flag)
                .unwrap()
                .cache_hit
        );
        assert!(tiny.is_empty());
    }
    let mut c = WaveformCache::new(1024 * 1024, 1).unwrap();
    c.get(&f.path, &f.probe, 256, None, &flag).unwrap();
    assert!(matches!(
        c.get(&f.path, &f.probe, 256, None, &AtomicBool::new(true)),
        Err(WaveformCacheError::Cancelled)
    ));
    assert!(WaveformCache::new(0, 1).is_err());
    assert!(WaveformCache::new(1, 65).is_err());
}

#[test]
fn import_inspection_discovers_metadata_and_preserves_original() {
    let f = Fixture::new();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        sungeki_media::probe_audio(&f.path, &cancel).unwrap(),
        f.probe
    );
    assert_eq!(fs::read(&f.path).unwrap(), f.bytes);
    assert!(matches!(
        sungeki_media::probe_audio(&f.path, &AtomicBool::new(true)),
        Err(WaveformCacheError::Cancelled)
    ));
    assert!(sungeki_media::probe_audio(std::path::Path::new("relative.wav"), &cancel).is_err());
    assert!(sungeki_media::probe_audio(&f.root, &cancel).is_err());
    let link = f.root.join("link.wav");
    std::os::unix::fs::symlink(&f.path, &link).unwrap();
    assert!(sungeki_media::probe_audio(&link, &cancel).is_err());
    fs::write(&f.path, b"not a WAV").unwrap();
    assert!(matches!(
        sungeki_media::probe_audio(&f.path, &cancel),
        Err(WaveformCacheError::Wav(_))
    ));
}

#[test]
fn inspection_rejects_oversized_source_before_allocating() {
    let f = Fixture::new();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&f.path)
        .unwrap()
        .set_len(sungeki_media::MAX_WAV_BYTES as u64 + 1)
        .unwrap();
    assert!(matches!(
        sungeki_media::probe_audio(&f.path, &AtomicBool::new(false)),
        Err(WaveformCacheError::Wav(sungeki_media::WavError::TooLarge))
    ));
}
