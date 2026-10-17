//! Development harness for the shared renderer; not a host/MCP endpoint.
#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, path::Path, sync::atomic::AtomicBool};
    use sungeki_render::{FramePlan, RenderRuntime, render_frame_staged};
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: preview_frame PROJECT_JSON RUNTIME_JSON EMPTY_DIRECTORY FRAME_INDEX".into(),
        );
    }
    let project = serde_json::from_slice(&fs::read(&args[0])?)?;
    let runtime: RenderRuntime = serde_json::from_slice(&fs::read(&args[1])?)?;
    let plan = FramePlan::new(&project, args[3].parse()?)?;
    let artifact = render_frame_staged(
        &plan,
        &runtime,
        Path::new(&args[2]),
        &AtomicBool::new(false),
    )?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
}
#[cfg(not(unix))]
fn main() {
    panic!("formal frame rendering requires Unix");
}
