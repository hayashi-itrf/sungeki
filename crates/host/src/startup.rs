use std::io::Write;
#[path = "speech_config_install.rs"]
pub(crate) mod speech_config_install;

// Best effort only: a disconnected launcher must never terminate the shared host.
fn phase(value: &str) {
    let _ = writeln!(std::io::stdout(), "VVM_STARTUP:{value}");
}

pub(crate) async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 3 && args[0] == "--install-speech-config" {
        let engine = args[1].to_str().ok_or("invalid engine name")?;
        let installed = speech_config_install::install(
            &crate::data::directory()?,
            engine,
            std::path::Path::new(&args[2]),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        serde_json::to_writer_pretty(
            std::io::stdout().lock(),
            &serde_json::json!({"configuration_path": installed}),
        )?;
        println!();
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--prepare-qwen-config" {
        let candidate = crate::qwen_originals::setup::prepare(
            std::path::Path::new(&args[1]),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        serde_json::to_writer_pretty(std::io::stdout().lock(), &candidate)?;
        println!();
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--check-qwen-config" {
        let report = crate::qwen_originals::preflight::check(
            std::path::Path::new(&args[1]),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        serde_json::to_writer_pretty(std::io::stdout().lock(), &report)?;
        println!();
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--prepare-irodori-config" {
        let candidate = crate::irodori_originals::setup::prepare(
            std::path::Path::new(&args[1]),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        serde_json::to_writer_pretty(std::io::stdout().lock(), &candidate)?;
        println!();
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--check-irodori-config" {
        let report = crate::irodori_originals::preflight::check(
            std::path::Path::new(&args[1]),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        serde_json::to_writer_pretty(std::io::stdout().lock(), &report)?;
        println!();
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--check-sbv2-config" {
        let report = crate::sbv2_originals::preflight::check(
            std::path::Path::new(&args[1]),
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        serde_json::to_writer_pretty(std::io::stdout().lock(), &report)?;
        println!();
        return Ok(());
    }
    if args.len() != 2 || args[0] != "--runtime-dir" {
        return Err("usage: sungeki-host --install-speech-config <sbv2|qwen|irodori> <absolute candidate file> | --runtime-dir <absolute directory> | --check-sbv2-config <absolute file> | --check-qwen-config <absolute file> | --prepare-qwen-config <absolute setup file> | --prepare-irodori-config <absolute setup file> | --check-irodori-config <absolute file>".into());
    }
    phase("runtime");
    let Some(reservation) = crate::endpoint::Reservation::acquire(std::path::Path::new(&args[1]))?
    else {
        // Another host owns the profile; the client retries its connection.
        return Ok(());
    };
    phase("data_directory");
    let data = crate::data::directory()?;
    phase("recovery");
    let sessions = crate::sessions::Sessions::open(&data.join("recovery.sqlite"))?;
    phase("listen");
    let endpoint = reservation.bind()?;
    crate::server::run(&endpoint.listener, sessions).await?;
    Ok(())
}
