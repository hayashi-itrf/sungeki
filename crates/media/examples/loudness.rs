//! Measure a known source without editing it, via the common owned-process runner.
use std::{fs::File, io::Read, path::Path, sync::atomic::AtomicBool, time::Duration};
use sungeki_media::{ExpectedAudio, MAX_WAV_BYTES, measure_loudness, verify_wav};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err(
            "usage: loudness <wav> <sample_rate> <channels> <expected_sha256> <ffmpeg> <work_root>"
                .into(),
        );
    }
    let mut bytes = vec![];
    File::open(&args[0])?
        .take(MAX_WAV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let wav = verify_wav(
        bytes,
        ExpectedAudio {
            sample_rate: args[1].parse()?,
            channels: args[2].parse()?,
        },
    )?;
    if wav.info().content_hash != args[3] {
        return Err("source fingerprint differs".into());
    }
    let result = measure_loudness(
        &wav,
        Path::new(&args[4]),
        Path::new(&args[5]),
        Duration::from_secs(60),
        &AtomicBool::new(false),
    )?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
