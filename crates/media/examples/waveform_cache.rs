//! Opt-in read-only cache check on a known WAV; does not launch TTS.
use std::{
    fs::File,
    io::Read,
    sync::{Arc, atomic::AtomicBool},
};
use sungeki_media::{ExpectedAudio, MAX_WAV_BYTES, WaveformCache, verify_wav};
use sungeki_project::{AudioProbe, TimeUs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: waveform_cache <wav> <sample_rate> <channels> <sha256>".into());
    }
    let mut bytes = Vec::new();
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
    let info = wav.info();
    if info.content_hash != args[3] {
        return Err("source fingerprint differs".into());
    }
    let probe = AudioProbe {
        encoding: None,
        sha256: info.content_hash.clone(),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        duration_us: TimeUs(info.duration_us),
        byte_length: wav.bytes().len() as u64,
    };
    let mut cache = WaveformCache::new(16 * 1024 * 1024, 4)?;
    let flag = AtomicBool::new(false);
    let first = cache.get(std::path::Path::new(&args[0]), &probe, 256, None, &flag)?;
    let repeated = cache.get(std::path::Path::new(&args[0]), &probe, 256, None, &flag)?;
    assert!(
        !first.cache_hit && repeated.cache_hit && Arc::ptr_eq(&first.waveform, &repeated.waveform)
    );
    let other = cache.get(std::path::Path::new(&args[0]), &probe, 512, None, &flag)?;
    assert!(!other.cache_hit);
    assert_eq!(first.waveform.rms, other.waveform.rms);
    println!(
        "{{\"sha256\":{:?},\"source_frames\":{},\"cache_reused\":true,\"different_resolution_miss\":true,\"retained_entries\":{},\"accounted_bytes\":{}}}",
        probe.sha256,
        probe.frames,
        cache.len(),
        cache.retained_bytes()
    );
    Ok(())
}
