#![cfg(unix)]
use std::{fs, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{Mp3DecodeError, ProcessSpec, decode_mp3, run_process};

#[test]
fn empty_input_and_precancel_do_not_create_work() {
    let flag = AtomicBool::new(false);
    assert!(matches!(
        decode_mp3(
            &[],
            &PathBuf::from("/unused"),
            &PathBuf::from("/unused"),
            Duration::from_secs(1),
            &flag
        ),
        Err(Mp3DecodeError::InputSize)
    ));
    flag.store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        decode_mp3(
            b"source",
            &PathBuf::from("/unused"),
            &PathBuf::from("/unused"),
            Duration::from_secs(1),
            &flag
        ),
        Err(Mp3DecodeError::Cancelled)
    ));
}

#[test]
#[ignore = "requires VVM_TEST_FFMPEG and SSD VVM_MEDIA_TEST_ROOT"]
fn real_mp3_gapless_decode_and_invalid_input_cleanup() {
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let root = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
        .join(format!("mp3-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let flag = AtomicBool::new(false);
    let args: Vec<_> = [
        "-nostdin",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=3",
        "-c:a",
        "libmp3lame",
        "-q:a",
        "2",
        "source.mp3",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let result = run_process(
        &ProcessSpec {
            program: &ffmpeg,
            args: &args,
            directory: &root,
            timeout: Duration::from_secs(30),
            max_output_bytes: 65536,
        },
        &flag,
    )
    .unwrap();
    assert!(result.succeeded());
    let original = fs::read(root.join("source.mp3")).unwrap();
    let first = decode_mp3(&original, &ffmpeg, &root, Duration::from_secs(30), &flag).unwrap();
    let second = decode_mp3(&original, &ffmpeg, &root, Duration::from_secs(30), &flag).unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(first.info().sample_rate, 48000);
    assert_eq!(first.info().channels, 2);
    assert_eq!(first.info().frames, 144000);
    assert!(first.info().is_float);
    assert_eq!(fs::read(root.join("source.mp3")).unwrap(), original);
    assert!(
        decode_mp3(
            b"not an MP3",
            &ffmpeg,
            &root,
            Duration::from_secs(30),
            &flag
        )
        .is_err()
    );
    let runtime = sungeki_media::AudioDecodeRuntime {
        ffmpeg: &ffmpeg,
        root: &root,
        timeout: Duration::from_secs(30),
    };
    let probe = sungeki_media::probe_mp3(&root.join("source.mp3"), &runtime, &flag).unwrap();
    assert_ne!(probe.sha256, first.info().content_hash);
    let mut cache = sungeki_media::WaveformCache::new(1024 * 1024, 4).unwrap();
    let wave = cache
        .get(&root.join("source.mp3"), &probe, 256, Some(&runtime), &flag)
        .unwrap();
    assert!(!wave.cache_hit);
    assert_eq!(wave.waveform.source_sha256, probe.sha256);
    assert_eq!(wave.waveform.source_frames, 144000);
    assert!(
        cache
            .get(&root.join("source.mp3"), &probe, 256, Some(&runtime), &flag)
            .unwrap()
            .cache_hit
    );

    let restored = sungeki_media::decode_audio_original(
        &root.join("source.mp3"),
        &probe,
        Some(&runtime),
        &flag,
    )
    .unwrap();
    assert_eq!(restored.bytes(), first.bytes());
    let captured_file =
        sungeki_media::decode_mp3_original_file(&root.join("source.mp3"), &probe, &runtime, &flag)
            .unwrap();
    assert_eq!(fs::read(captured_file.path()).unwrap(), first.bytes());
    let kept = root.join("kept.wav");
    fs::hard_link(captured_file.path(), &kept).unwrap();
    drop(captured_file);
    assert_eq!(fs::read(&kept).unwrap(), first.bytes());
    fs::remove_file(kept).unwrap();
    let mut changed = probe.clone();
    changed.encoding = Some(sungeki_project::CompressedAudioEncoding::Mp3 {
        decode_version: 1,
        decoded_sha256: "0".repeat(64),
    });
    assert!(matches!(
        sungeki_media::decode_audio_original(
            &root.join("source.mp3"),
            &changed,
            Some(&runtime),
            &flag
        ),
        Err(sungeki_media::AudioDecodeError::Mismatch)
    ));
    assert!(matches!(
        sungeki_media::decode_mp3_original_file(
            &root.join("source.mp3"),
            &changed,
            &runtime,
            &flag
        ),
        Err(sungeki_media::AudioDecodeError::Mismatch)
    ));
    let mut short = probe.clone();
    short.frames -= 48000;
    short.duration_us.0 -= 1_000_000;
    assert!(matches!(
        sungeki_media::decode_mp3_original_file(&root.join("source.mp3"), &short, &runtime, &flag),
        Err(sungeki_media::AudioDecodeError::Mismatch)
    ));
    fs::write(root.join("source.mp3"), b"changed original").unwrap();
    assert!(matches!(
        cache.get(&root.join("source.mp3"), &probe, 256, Some(&runtime), &flag),
        Err(sungeki_media::WaveformCacheError::SourceChanged)
    ));

    assert!(matches!(
        sungeki_media::decode_audio_original(
            &root.join("source.mp3"),
            &probe,
            Some(&runtime),
            &flag
        ),
        Err(sungeki_media::AudioDecodeError::Source(_))
    ));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_file(root.join("source.mp3")).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn decode_output_overflow_and_timeout_never_return_partial_audio() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("mp3-fault-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let worker = root.join("worker.sh");
    let flag = AtomicBool::new(false);
    fs::write(&worker, "#!/bin/sh\nhead -c 67108864 /dev/zero\n").unwrap();
    fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        decode_mp3(b"captured", &worker, &root, Duration::from_secs(10), &flag),
        Err(Mp3DecodeError::Stopped(
            sungeki_media::StopReason::OutputLimit
        ))
    ));
    fs::write(&worker, "#!/bin/sh\nsleep 10\n").unwrap();
    assert!(matches!(
        decode_mp3(
            b"captured",
            &worker,
            &root,
            Duration::from_millis(40),
            &flag
        ),
        Err(Mp3DecodeError::Stopped(sungeki_media::StopReason::Deadline))
    ));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_file(worker).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
#[ignore = "requires VVM_TEST_MP3, VVM_TEST_FFMPEG and SSD VVM_MEDIA_TEST_ROOT"]
fn real_legacy_bgm_decodes_without_modifying_original() {
    let source = PathBuf::from(std::env::var_os("VVM_TEST_MP3").unwrap());
    let ffmpeg = PathBuf::from(std::env::var_os("VVM_TEST_FFMPEG").unwrap());
    let root = PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap());
    let bytes = fs::read(&source).unwrap();
    let wav = decode_mp3(
        &bytes,
        &ffmpeg,
        &root,
        Duration::from_secs(60),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert!(wav.info().duration_us > 90_000_000 && wav.info().duration_us < 93_000_000);
    println!("decoded legacy BGM: {:?}", wav.info());
}
