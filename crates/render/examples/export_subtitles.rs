//! Development smoke test for the shared exporter; GUI/MCP wiring is separate.
use std::{env, fs, path::PathBuf};
use sungeki_render::{SubtitleFormat, subtitle_document};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: export_subtitles <project.vvm.json> <new-output-directory>".into());
    }
    let project = serde_json::from_slice(&fs::read(&args[0])?)?;
    let srt = subtitle_document(&project, SubtitleFormat::Srt, false)?;
    let vtt = subtitle_document(&project, SubtitleFormat::WebVtt, false)?;
    let destination = PathBuf::from(&args[1]);
    fs::create_dir(&destination)?;
    fs::write(destination.join("captions.srt"), srt)?;
    fs::write(destination.join("captions.vtt"), vtt)?;
    println!("{}", destination.display());
    Ok(())
}
