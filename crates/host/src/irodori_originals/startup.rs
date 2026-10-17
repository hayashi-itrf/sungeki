//! Managed startup only. Callers must reserve/journal engine resources before this.
use crate::{
    irodori_originals::{Measured, binding::Binding},
    sbv2_process::OwnedProcess,
};
use std::{
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use sungeki_project::{
    IrodoriAudioSource as Selection, IrodoriIdentity as RuntimeIdentity,
    IrodoriSettings as Settings,
};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::irodori::{Catalog, Error, IrodoriClient};

pub(crate) struct Request {
    pub progress: crate::speech_state::progress::Tracker,
    pub binding: Binding,
    pub selection: Selection,
    pub port: u16,
    pub settings: Settings,
    pub identity: RuntimeIdentity,
}
pub(crate) struct Ready {
    pub process: OwnedProcess,
    pub catalog: Catalog,
    pub measured: Measured,
}
pub(crate) struct Failure {
    pub error: ApiError,
    /// False includes a lost worker result. Never release quarantine on that basis.
    pub cleanup_confirmed: bool,
    pub cleanup_error: Option<ApiError>,
    /// Retain ownership on stop timeout so a caller can retry settlement.
    pub unsettled: Option<OwnedProcess>,
}
impl Failure {
    pub(super) fn before_launch(error: ApiError) -> Self {
        Self {
            error,
            cleanup_confirmed: true,
            cleanup_error: None,
            unsettled: None,
        }
    }
}
fn unavailable(message: impl ToString) -> ApiError {
    ApiError::new(ErrorCode::EngineUnavailable, message.to_string())
}
fn cancelled(cancel: &AtomicBool) -> Result<(), ApiError> {
    if cancel.load(Ordering::Acquire) {
        Err(ApiError::new(
            ErrorCode::Cancelled,
            "Irodori起動を中止しました",
        ))
    } else {
        Ok(())
    }
}
/// Owns a prebound socket before spawn; a pre-existing server is never adopted.
/// Readiness means catalog + remeasured configured files, not loader attestation.
pub(crate) async fn start(
    request: Request,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<Ready, Failure> {
    if request.port < 1024 || timeout.is_zero() || timeout > Duration::from_secs(600) {
        return Err(Failure::before_launch(unavailable(
            "Irodori起動待機時間が不正です",
        )));
    }
    request
        .progress
        .set(crate::speech_state::progress::Stage::Inspect);
    let request = Arc::new(request);
    let context = request.clone();
    let flag = cancel.clone();
    let launched = tokio::task::spawn_blocking(move || {
        cancelled(&flag)?;
        let address = std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, context.port);
        let listener = TcpListener::bind(address).map_err(ApiError::io)?;
        let command = context.binding.checked_command(&context.identity, &flag)?.0;
        cancelled(&flag)?;
        context
            .progress
            .set(crate::speech_state::progress::Stage::Start);
        OwnedProcess::spawn_listening(command, listener).map_err(ApiError::io)
    })
    .await;
    let process = match launched {
        Ok(Ok(process)) => process,
        Ok(Err(error)) => return Err(Failure::before_launch(error)),
        Err(_) => {
            return Err(Failure {
                error: unavailable("Irodori起動ワーカーが終了しました"),
                cleanup_confirmed: false,
                cleanup_error: None,
                unsettled: None,
            });
        }
    };
    let progress = request.progress.clone();
    let result = verify_ready(&process, request, cancel, timeout).await;
    match result {
        Ok((catalog, measured)) => Ok(Ready {
            process,
            catalog,
            measured,
        }),
        Err(error) => {
            progress.set(crate::speech_state::progress::Stage::Stop);
            Err(cleanup(process, error).await)
        }
    }
}
async fn verify_ready(
    process: &OwnedProcess,
    request: Arc<Request>,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(Catalog, Measured), ApiError> {
    let client = IrodoriClient::new(
        &format!("http://127.0.0.1:{}", request.port),
        Duration::from_secs(1),
    )
    .map_err(unavailable)?;
    let catalog = wait_catalog(process, &client, &cancel, timeout).await?;
    let selected = catalog
        .select(&request.settings, &request.identity)
        .map_err(unavailable)?;
    if selected.audio_source() != &request.selection {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "Irodoriの起動後のモデル情報が変更されています",
        ));
    }
    request
        .progress
        .set(crate::speech_state::progress::Stage::Recheck);
    let flag = cancel.clone();
    let measured = tokio::task::spawn_blocking(move || {
        // Recheck launch paths/interpreter links as well as manifest contents.
        request
            .binding
            .checked_command(&request.identity, &flag)
            .map(|(_, measured)| measured)
    })
    .await
    .map_err(|_| unavailable("Irodori原本の再検査ワーカーが終了しました"))??;
    cancelled(&cancel)?;
    if process.has_exited().map_err(ApiError::io)? {
        return Err(unavailable("Irodoriが準備完了前に終了しました"));
    }
    Ok((catalog, measured))
}
async fn wait_catalog(
    process: &OwnedProcess,
    client: &IrodoriClient,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<Catalog, ApiError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        cancelled(cancel)?;
        if process.has_exited().map_err(ApiError::io)? {
            return Err(unavailable("Irodoriが起動中に終了しました"));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(unavailable("Irodoriの起動待機がタイムアウトしました"));
        }
        let result = tokio::time::timeout_at(deadline, async {
            let health = client.health(cancel).await?;
            if health.busy {
                return Err(Error::NotReady);
            }
            client.catalog(cancel).await
        })
        .await;
        cancelled(cancel)?;
        if process.has_exited().map_err(ApiError::io)? {
            return Err(unavailable("Irodoriが起動中に終了しました"));
        }
        let result = result.map_err(|_| unavailable("Irodoriの起動待機がタイムアウトしました"))?;
        match result {
            Ok(catalog) => return Ok(catalog),
            Err(Error::Connection | Error::Timeout | Error::Http(503) | Error::NotReady) => {}
            Err(Error::Cancelled) => {
                return Err(ApiError::new(
                    ErrorCode::Cancelled,
                    "Irodori起動を中止しました",
                ));
            }
            Err(error) => return Err(unavailable(error)),
        }
        tokio::time::sleep_until(
            deadline.min(tokio::time::Instant::now() + Duration::from_millis(100)),
        )
        .await;
    }
}
async fn cleanup(mut process: OwnedProcess, error: ApiError) -> Failure {
    let result = tokio::task::spawn_blocking(move || {
        let result = process.stop(Duration::from_secs(2));
        (process, result)
    })
    .await;
    match result {
        Ok((_, Ok(_))) => Failure::before_launch(error),
        Ok((process, Err(cleanup_error))) => Failure {
            error,
            cleanup_confirmed: false,
            cleanup_error: Some(ApiError::io(cleanup_error)),
            unsettled: Some(process),
        },
        Err(_) => Failure {
            error,
            cleanup_confirmed: false,
            cleanup_error: Some(unavailable("Irodori停止ワーカーが終了しました")),
            unsettled: None,
        },
    }
}

#[cfg(test)]
mod tests;
