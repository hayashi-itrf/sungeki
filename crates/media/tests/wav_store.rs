use std::{fs, io::Cursor};
use sungeki_media::{ExpectedAudio, store_wav_new, verify_wav};

#[test]
fn published_audio_is_complete_and_existing_destinations_are_never_replaced() {
    let root = std::env::temp_dir().join(format!("vvm-wav-store-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let destination = root.join("speech.wav");
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut bytes,
            hound::WavSpec {
                channels: 1,
                sample_rate: 24000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        writer.write_sample(1i16).unwrap();
        writer.finalize().unwrap();
    }
    let audio = verify_wav(
        bytes.into_inner(),
        ExpectedAudio {
            sample_rate: 24000,
            channels: 1,
        },
    )
    .unwrap();
    store_wav_new(&audio, &destination).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), audio.bytes());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    let verified = verify_wav(
        fs::read(&destination).unwrap(),
        ExpectedAudio {
            sample_rate: 24000,
            channels: 1,
        },
    )
    .unwrap();
    assert_eq!(verified.info(), audio.info());
    fs::write(&destination, b"existing file must survive").unwrap();
    let error = store_wav_new(&audio, &destination).unwrap_err();
    assert!(!error.published);
    assert_eq!(error.source.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read(&destination).unwrap(),
        b"existing file must survive"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert!(
        !store_wav_new(&audio, &root.join("missing/clip.wav"))
            .unwrap_err()
            .published
    );
    assert!(
        !store_wav_new(&audio, std::path::Path::new("relative.wav"))
            .unwrap_err()
            .published
    );
    #[cfg(unix)]
    {
        let target = root.join("target.wav");
        fs::write(&target, b"symlink target").unwrap();
        let link = root.join("linked.wav");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(!store_wav_new(&audio, &link).unwrap_err().published);
        assert_eq!(fs::read(target).unwrap(), b"symlink target");
    }
    fs::remove_dir_all(&root).unwrap();
}
