//! Read-only measurement of a frozen pre-master mix using its recorded hash.
use std::{path::Path, sync::atomic::AtomicBool, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: mix_loudness <mix.wav> <expected_sha256> <ffmpeg> <work_root>".into());
    }
    let result = sungeki_media::measure_mix_loudness(
        Path::new(&args[0]),
        &args[1],
        Path::new(&args[2]),
        Path::new(&args[3]),
        Duration::from_secs(300),
        &AtomicBool::new(false),
    )?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
