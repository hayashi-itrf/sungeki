#[cfg(test)]
mod audio_probe_tests;
mod client;
mod discovery;
#[cfg(test)]
mod speech_preparation_tests;
mod startup_progress;

pub use client::HostClient;
pub use discovery::HostOptions;
use sungeki_protocol::{ApiError, HostInfo};

pub async fn get_capabilities(client_name: &str) -> Result<HostInfo, ApiError> {
    let options = HostOptions::from_environment()?;
    let mut client = HostClient::connect_or_start(&options, client_name).await?;
    client.capabilities().await
}
