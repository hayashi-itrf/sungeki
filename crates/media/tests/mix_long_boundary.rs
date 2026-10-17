#![cfg(unix)]
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use sungeki_media::*;
use sungeki_project::{FrameRate, TimeUs};

#[test]
#[ignore = "writes/captures a sparse 2.76GB fixture; requires SSD VVM_MEDIA_TEST_ROOT"]
fn two_hour_fractional_frame_and_aac_padding_reach_worker_after_full_validation() {
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let root = Root(
        PathBuf::from(std::env::var_os("VVM_MEDIA_TEST_ROOT").unwrap())
            .join(format!("long-mix-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&root.0).unwrap();
    let work = root.0.join("work");
    fs::create_dir(&work).unwrap();
    let path = root.0.join("two-hours.wav");
    // Independent frame calculation: ceil(7200 * 30000/1001) = 215785.
    // ceil(215785 * 1001/30000 * 48000) = 345601256; plus one AAC frame.
    let frames = 345_601_256u64 + 1024;
    let bytes = frames * 8;
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&((bytes + 36) as u32).to_le_bytes())
        .unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&3u16.to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&48000u32.to_le_bytes()).unwrap();
    file.write_all(&384000u32.to_le_bytes()).unwrap();
    file.write_all(&8u16.to_le_bytes()).unwrap();
    file.write_all(&32u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&(bytes as u32).to_le_bytes()).unwrap();
    file.set_len(bytes + 44).unwrap();
    drop(file);
    let mut digest = Sha256::new();
    let mut input = fs::File::open(&path).unwrap();
    let mut buffer = [0; 65536];
    loop {
        let n = input.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    println!("fixture hash complete");
    let hash = format!("{:x}", digest.finalize());
    let flag = AtomicBool::new(false);
    assert!(matches!(
        measure_mix_loudness(
            &path,
            &hash,
            Path::new("/usr/bin/false"),
            &work,
            Duration::from_secs(600),
            &flag
        ),
        Err(LoudnessError::InvalidReport)
    ));
    println!("raw two-hour bound rejected extended WAV");
    let limits = MixFrameLimit::for_video(
        TimeUs(7_200_000_000),
        &FrameRate {
            numerator: 30000,
            denominator: 1001,
        },
        true,
    )
    .unwrap();
    // A failing worker is intentional: reaching it proves the real WAV, hash,
    // frame count and every finite sample passed. This is not a loudness result.
    let result = measure_render_mix_loudness(
        &path,
        &hash,
        Path::new("/usr/bin/false"),
        &work,
        Duration::from_secs(600),
        &flag,
        limits,
    );
    assert!(
        matches!(result, Err(LoudnessError::Worker(_))),
        "{result:?}"
    );
    assert_eq!(fs::read_dir(&work).unwrap().count(), 0);
    println!("validated {frames} frames ({bytes} PCM bytes), capture removed");
}
