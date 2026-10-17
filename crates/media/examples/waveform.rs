//! Opt-in inspection of an existing WAV. No engine, Project edits, or source writes.
use std::{fs::File, io::Read, sync::atomic::AtomicBool};
use sungeki_media::{ExpectedAudio, MAX_WAV_BYTES, analyze_waveform, verify_wav};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "usage: waveform <wav> <sample_rate> <channels> <base_frames> <expected_sha256>".into(),
        );
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
    if wav.info().content_hash != args[4] {
        return Err("source fingerprint differs".into());
    }
    let result = analyze_waveform(&wav, args[3].parse()?, &AtomicBool::new(false))?;
    println!(
        "{{\"version\":{},\"source_sha256\":{:?},\"source_frames\":{},\"sample_rate\":{},\"rms\":{:?},\"sample_peak\":{:?},\"levels\":[",
        result.version,
        result.source_sha256,
        result.source_frames,
        result.sample_rate,
        result.rms,
        result.sample_peak
    );
    for (index, level) in result.levels().iter().enumerate() {
        if index > 0 {
            println!(",");
        }
        print!(
            "{{\"frames_per_bucket\":{},\"bucket_count\":{},\"channel_windows\":[",
            level.frames_per_bucket,
            level.bucket_count()
        );
        for channel in 0..usize::from(result.channels) {
            if channel > 0 {
                print!(",");
            }
            let window = result.window(index, channel, 0, 4096)?;
            print!(
                "{{\"start_frame\":{},\"end_frame\":{},\"peaks\":{:?}}}",
                window.start_frame,
                window.end_frame,
                window
                    .peaks
                    .iter()
                    .map(|p| [p.min, p.max])
                    .collect::<Vec<_>>()
            );
        }
        print!("]}}");
    }
    println!("]}}");
    Ok(())
}
