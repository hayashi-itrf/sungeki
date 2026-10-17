//! Read-only diagnostic: lipsync <WAV> <sample rate> <channels>.
use std::{fs::File, io::Read, sync::atomic::AtomicBool};
use sungeki_media::{ExpectedAudio, MAX_WAV_BYTES, analyze_lipsync, verify_wav};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: lipsync <WAV> <sample rate> <channels>".into());
    }
    let mut bytes = vec![];
    File::open(&args[1])?
        .take(MAX_WAV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let wav = verify_wav(
        bytes,
        ExpectedAudio {
            sample_rate: args[2].parse()?,
            channels: args[3].parse()?,
        },
    )?;
    let result = analyze_lipsync(&wav, Default::default(), &AtomicBool::new(false))?;
    println!(
        "source\t{}\t{}\t{}\t{}\t{}\t{}",
        result.version,
        result.source_sha256,
        result.sample_rate,
        result.source_frames,
        result.peak_rms,
        result.threshold_rms
    );
    for span in result.open {
        println!("open\t{}\t{}", span.start_frame, span.end_frame);
    }
    Ok(())
}
