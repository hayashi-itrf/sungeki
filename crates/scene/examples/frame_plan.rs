//! Development inspection of a Project saved through the new MCP. No edits.
use std::{env, fs};
use sungeki_project::Project;
use sungeki_scene::ScenePlan;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: frame_plan PROJECT FRAME_INDEX".into());
    }
    let project: Project = serde_json::from_slice(&fs::read(&args[1])?)?;
    let plan = ScenePlan::new(&project)?;
    let frame = args[2].parse::<u64>()?;
    let layers = plan.frame(frame).ok_or("frame outside project")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "canvas": plan.canvas, "frame_index": frame,
            "time_us": plan.canvas.fps.frame_time(frame), "layers": layers,
        }))?
    );
    Ok(())
}
