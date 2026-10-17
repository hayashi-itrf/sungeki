use sungeki_media::{LoudnessAnalysis, propose_speech_gain};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: gain_proposal <analysis.json> <target_lufs> <max_true_peak_db>".into());
    }
    let input: LoudnessAnalysis = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    println!(
        "{}",
        serde_json::to_string(&propose_speech_gain(
            &input,
            args[1].parse()?,
            args[2].parse()?
        )?)?
    );
    Ok(())
}
