mod audio_preview_dispatch;
mod build_dispatch;
mod collection_dispatch;
mod context;
mod dispatch;
mod frame_preview_dispatch;
mod loudness_dispatch;
mod presets;
mod psd_dispatch;
mod render_dispatch;
mod server;
mod tools;

use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = server::BootstrapServer::default().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod sbv2_tests;
