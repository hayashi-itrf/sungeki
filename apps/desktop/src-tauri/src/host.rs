use sungeki_protocol::{ApiError, HostInfo};

#[tauri::command]
pub async fn get_host_capabilities() -> Result<HostInfo, ApiError> {
    let _guard = crate::exit::allow_request().await?;
    let options = sungeki_host_client::HostOptions::from_environment()?;
    let mut client = sungeki_host_client::HostClient::connect_or_start(&options, "Desktop").await?;
    crate::exit::retain_connection(&options).await?;
    client.capabilities().await
}

#[tauri::command]
pub async fn session_request(
    request: sungeki_protocol::SessionRequest,
) -> Result<sungeki_protocol::SessionResponse, ApiError> {
    let _guard = crate::exit::allow_request().await?;
    let options = sungeki_host_client::HostOptions::from_environment()?;
    let mut client = sungeki_host_client::HostClient::connect_or_start(&options, "Desktop").await?;
    crate::exit::retain_connection(&options).await?;
    client.session(request).await
}

#[tauri::command]
pub async fn watch_sessions(
    after: Option<sungeki_protocol::EventCursor>,
    wait_ms: u32,
) -> Result<sungeki_protocol::EventBatch, ApiError> {
    let _guard = crate::exit::allow_request().await?;
    let options = sungeki_host_client::HostOptions::from_environment()?;
    let mut client =
        sungeki_host_client::HostClient::connect_or_start(&options, "Desktop events").await?;
    crate::exit::retain_connection(&options).await?;
    client.watch(after, wait_ms.min(1_000)).await
}
