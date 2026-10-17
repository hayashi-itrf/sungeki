//! Managed startup only. Callers must reserve/journal engine resources before this.
use crate::{
    sbv2_originals::{Binding, Measured},
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
use sungeki_project::{Sbv2Identity, Sbv2Settings};
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::sbv2::{Catalog, Error, Sbv2Client};

pub(crate) struct Request {
    pub progress: crate::speech_state::progress::Tracker,
    pub binding: Binding,
    pub catalog: Catalog,
    pub settings: Sbv2Settings,
    pub identity: Sbv2Identity,
    pub cpu_only: bool,
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
    fn before_launch(error: ApiError) -> Self {
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
            "SBV2起動を中止しました",
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
    if timeout.is_zero() || timeout > Duration::from_secs(600) {
        return Err(Failure::before_launch(unavailable(
            "SBV2起動待機時間が不正です",
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
        let address = context.binding.managed_address()?;
        let listener = TcpListener::bind(address).map_err(ApiError::io)?;
        let command = context.binding.launch_command(
            &context.catalog,
            &context.settings,
            &context.identity,
            context.cpu_only,
            &flag,
        )?;
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
                error: unavailable("SBV2起動ワーカーが終了しました"),
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
    let client =
        Sbv2Client::new(&request.binding.endpoint, Duration::from_secs(1)).map_err(unavailable)?;
    let catalog = wait_catalog(process, &client, &cancel, timeout).await?;
    if catalog.select(&request.settings).map_err(unavailable)?
        != request
            .catalog
            .select(&request.settings)
            .map_err(unavailable)?
    {
        return Err(ApiError::new(
            ErrorCode::ExternalModification,
            "SBV2の起動後のモデル情報が変更されています",
        ));
    }
    let observed = catalog.clone();
    request
        .progress
        .set(crate::speech_state::progress::Stage::Recheck);
    let flag = cancel.clone();
    let measured = tokio::task::spawn_blocking(move || {
        request.binding.inspect_selected(
            &request.binding.endpoint,
            &observed,
            &request.settings,
            &request.identity,
            &flag,
        )
    })
    .await
    .map_err(|_| unavailable("SBV2原本の再検査ワーカーが終了しました"))??;
    cancelled(&cancel)?;
    if process.has_exited().map_err(ApiError::io)? {
        return Err(unavailable("SBV2が準備完了前に終了しました"));
    }
    Ok((catalog, measured))
}
async fn wait_catalog(
    process: &OwnedProcess,
    client: &Sbv2Client,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<Catalog, ApiError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        cancelled(cancel)?;
        if process.has_exited().map_err(ApiError::io)? {
            return Err(unavailable("SBV2が起動中に終了しました"));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(unavailable("SBV2の起動待機がタイムアウトしました"));
        }
        let result = tokio::time::timeout_at(deadline, client.catalog(cancel)).await;
        cancelled(cancel)?;
        if process.has_exited().map_err(ApiError::io)? {
            return Err(unavailable("SBV2が起動中に終了しました"));
        }
        let result = result.map_err(|_| unavailable("SBV2の起動待機がタイムアウトしました"))?;
        match result {
            Ok(catalog) => return Ok(catalog),
            Err(Error::Connection | Error::Timeout | Error::Http(503)) => {}
            Err(Error::Cancelled) => {
                return Err(ApiError::new(
                    ErrorCode::Cancelled,
                    "SBV2起動を中止しました",
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
            cleanup_error: Some(unavailable("SBV2停止ワーカーが終了しました")),
            unsettled: None,
        },
    }
}
