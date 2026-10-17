//! Managed exchange primitive. Caller reserves/journals resources before entry.
//! No Project mutation or publishing: success is verified audio after owned stop.
use super::{
    binding::Binding,
    startup::{self, Failure, Request},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use sungeki_media::VerifiedWav;
use sungeki_protocol::{ApiError, ErrorCode};
use sungeki_speech::qwen::{Error, PreparedQwenInput, QwenClient};
fn invalid() -> ApiError {
    ApiError::new(
        ErrorCode::ValidationFailed,
        "Qwen合成入力と起動設定が一致しません",
    )
}
fn cancelled(flag: &AtomicBool) -> Result<(), ApiError> {
    if flag.load(Ordering::Acquire) {
        Err(ApiError::new(
            ErrorCode::Cancelled,
            "Qwen合成を取り消しました",
        ))
    } else {
        Ok(())
    }
}
fn api_error(error: Error) -> ApiError {
    let code = match error {
        Error::Cancelled => ErrorCode::Cancelled,
        Error::Configuration => ErrorCode::ValidationFailed,
        Error::ModelChanged => ErrorCode::ExternalModification,
        _ => ErrorCode::EngineUnavailable,
    };
    ApiError::new(code, error.to_string())
}
pub(crate) async fn run(
    request: Request,
    input: PreparedQwenInput,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<VerifiedWav, Failure> {
    if timeout.is_zero()
        || timeout > Duration::from_secs(300)
        || input.identity() != Some(&request.identity)
        || input.settings() != &request.settings
        || input.selection() != &request.selection
    {
        return Err(Failure::before_launch(invalid()));
    }
    let progress = request.progress.clone();
    let binding = request.binding.clone();
    let port = request.port;
    // Cold model loading has a different budget from an individual synthesis
    // request. Keep cancellation active throughout the bounded startup wait.
    let ready = startup::start(request, cancel.clone(), Duration::from_secs(600)).await?;
    progress.set(crate::speech_state::progress::Stage::Synthesize);
    let response = synthesize(
        &ready,
        binding,
        port,
        input,
        cancel.clone(),
        timeout,
        &progress,
    )
    .await;
    progress.set(crate::speech_state::progress::Stage::Stop);
    // Stop on every normal completion path, including failed/cancelled requests.
    // Dropped futures rely on OwnedProcess best effort; durable quarantine remains.
    let mut process = ready.process;
    let stopped = tokio::task::spawn_blocking(move || {
        let result = process.stop(Duration::from_secs(2));
        (process, result)
    })
    .await;
    match stopped {
        Ok((_, Ok(_))) => {
            cancelled(&cancel).map_err(Failure::before_launch)?;
            response.map_err(Failure::before_launch)
        }
        Ok((process, Err(error))) => {
            let cleanup = ApiError::io(error);
            Err(Failure {
                error: response.err().unwrap_or_else(|| cleanup.clone()),
                cleanup_confirmed: false,
                cleanup_error: Some(cleanup),
                unsettled: Some(process),
            })
        }
        Err(_) => {
            let cleanup = ApiError::new(
                ErrorCode::EngineUnavailable,
                "Qwen停止ワーカーが終了しました",
            );
            Err(Failure {
                error: response.err().unwrap_or_else(|| cleanup.clone()),
                cleanup_confirmed: false,
                cleanup_error: Some(cleanup),
                unsettled: None,
            })
        }
    }
}
async fn synthesize(
    ready: &startup::Ready,
    binding: Binding,
    port: u16,
    input: PreparedQwenInput,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
    progress: &crate::speech_state::progress::Tracker,
) -> Result<VerifiedWav, ApiError> {
    cancelled(&cancel)?;
    let client =
        QwenClient::new(&format!("http://127.0.0.1:{port}"), timeout).map_err(api_error)?;
    let exchange = client.exchange_prepared(&input, &cancel);
    tokio::pin!(exchange);
    let bytes = loop {
        if ready.process.has_exited().map_err(ApiError::io)? {
            return Err(ApiError::new(
                ErrorCode::EngineUnavailable,
                "Qwenが合成中に終了しました",
            ));
        }
        tokio::select! {
            result=&mut exchange=>break result.map_err(|e|api_error(e.error))?,
            _=tokio::time::sleep(Duration::from_millis(100))=>cancelled(&cancel)?,
        }
    };
    cancelled(&cancel)?;
    progress.set(crate::speech_state::progress::Stage::Verify);
    let input = input.clone();
    let flag = cancel.clone();
    tokio::task::spawn_blocking(move || {
        binding.checked_command(port, input.identity().ok_or_else(invalid)?, &flag)?;
        let audio = input.verify_audio(bytes).map_err(|_| {
            ApiError::new(
                ErrorCode::ValidationFailed,
                "Qwen音声の形式または内容が不正です",
            )
        })?;
        cancelled(&flag)?;
        Ok(audio)
    })
    .await
    .map_err(|_| {
        ApiError::new(
            ErrorCode::EngineUnavailable,
            "Qwen音声検査ワーカーが終了しました",
        )
    })?
}
