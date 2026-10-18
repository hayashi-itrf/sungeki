use crate::sessions::Sessions;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use sungeki_protocol::{
    framing::{MAX_FRAME_BYTES, MAX_HANDSHAKE_BYTES, read_frame_limited, write_frame},
    *,
};
use tokio::{
    net::{UnixListener, UnixStream},
    task::JoinSet,
    time::{Instant, timeout},
};

pub async fn run(listener: &UnixListener, sessions: Sessions) -> Result<(), ApiError> {
    let profile_inspection_cancel = sessions.profile_inspection_cancel.clone();
    let build_runtime = sessions.build_runtime.clone();
    let build_changed = build_runtime.wake.clone();
    let active_build = build_runtime.active.clone();
    let loudness_runtime = sessions.loudness_runtime.clone();
    let loudness_changed = loudness_runtime.wake.clone();
    let active_loudness = loudness_runtime.active.clone();
    let render_runtime = sessions.render_runtime.clone();
    let render_changed = render_runtime.wake.clone();
    let active_render = render_runtime.active.clone();
    let speech_changed = sessions.speech_runtime.idle_wake.clone();
    let active_speech = sessions.speech_runtime.active.clone();
    let jobs_changed = sessions.review_runtime.idle_wake.clone();
    let active_jobs = sessions.review_runtime.active.clone();
    let host_instance_id = sessions.events.cursor().host_instance_id;
    let sessions = Arc::new(Mutex::new(sessions));
    crate::collection_execute::recovery::launch(sessions.clone())
        .map_err(|e| ApiError::new(ErrorCode::IoError, format!("収集の復旧開始: {e:?}")))?;
    let info = Arc::new(HostInfo {
        application: APPLICATION.into(),
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: PROTOCOL_MINOR,
        build_id: BUILD_ID.into(),
        host_instance_id,
        capabilities: Capabilities {
            editing: true,
            persistence: true,
            speech: false,
            rendering: false,
            vrm: false,
        },
    });
    let exit = Arc::new(crate::exit_control::Control::default());
    let mut tasks = JoinSet::new();
    let (closing, closing_rx) = tokio::sync::watch::channel(false);
    let mut idle_since = Instant::now();
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(ApiError::io)?;
    loop {
        if exit.stop.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::select! {
            _ = exit.wake.notified() => {},
            _ = terminate.recv() => break,
            _ = build_changed.notified() => { idle_since = Instant::now(); },
            _ = loudness_changed.notified() => { idle_since = Instant::now(); },
            _ = render_changed.notified() => { idle_since = Instant::now(); },
            _ = speech_changed.notified() => { idle_since = Instant::now(); },
            _ = jobs_changed.notified() => { idle_since = Instant::now(); },
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep_until(idle_since + Duration::from_secs(if exit.fast_idle.load(std::sync::atomic::Ordering::SeqCst) { 0 } else { 30 })), if tasks.is_empty() && active_build.load(std::sync::atomic::Ordering::SeqCst) == 0 && active_loudness.load(std::sync::atomic::Ordering::SeqCst) == 0 && active_render.load(std::sync::atomic::Ordering::SeqCst) == 0 && active_jobs.load(std::sync::atomic::Ordering::SeqCst) == 0 && active_speech.load(std::sync::atomic::Ordering::SeqCst) == 0 => break,
            Some(_) = tasks.join_next(), if !tasks.is_empty() => { idle_since = Instant::now(); },
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(ApiError::io)?;
                if tasks.len() >= 32 { continue; }
                let Some(connection) = exit.connect(stream.peer_cred().ok().and_then(|c| c.pid())) else { continue; };
                let exit = exit.clone();
                let info = info.clone();
                let sessions = sessions.clone();
                let closing = closing_rx.clone();
                tasks.spawn(async move {
                    if let Err(error) = serve(stream, &info, sessions, closing, exit, connection).await {
                        eprintln!("host connection: {error}");
                    }
                });
                idle_since = Instant::now();
            }
        }
    }
    // Stop accepting the next request, but let active responses be delivered.
    profile_inspection_cancel.store(true, std::sync::atomic::Ordering::Release);
    crate::http_engine::begin_shutdown();
    let _ = closing.send(true);
    if let Err(error) = crate::build_record::shutdown(&sessions, Duration::from_secs(60)).await {
        eprintln!("build shutdown: {error}");
    }
    tokio::join!(
        crate::exit_cleanup::cancel_and_wait(&sessions),
        loudness_runtime.shutdown(),
        render_runtime.shutdown()
    );
    // Media admission settles before its blocking result reaches the connection.
    // Give that result the same bounded write window as an ordinary response.
    let _ = timeout(Duration::from_secs(2), async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}

async fn serve(
    mut stream: UnixStream,
    info: &HostInfo,
    sessions: Arc<Mutex<Sessions>>,
    mut closing: tokio::sync::watch::Receiver<bool>,
    exit: Arc<crate::exit_control::Control>,
    connection: crate::exit_control::Connection,
) -> Result<(), ApiError> {
    // SAFETY: geteuid has no arguments or failure mode.
    if stream.peer_cred().map_err(ApiError::io)?.uid() != unsafe { libc::geteuid() } {
        return Err(ApiError::new(
            ErrorCode::InvalidRuntime,
            "別ユーザーからの接続です",
        ));
    }
    let mut ready = false;
    loop {
        if *closing.borrow() {
            return Ok(());
        }
        let limit = if ready {
            MAX_FRAME_BYTES
        } else {
            MAX_HANDSHAKE_BYTES
        };
        let incoming = tokio::select! {
            biased;
            _ = closing.changed() => return Ok(()),
            incoming = timeout(Duration::from_secs(10), read_frame_limited(&mut stream, limit)) => incoming,
        };
        let request: Request = match incoming {
            Ok(Ok(request)) => request,
            // EOF, invalid framing and idle connections are never reused.
            _ => return Ok(()),
        };
        // Keep the guard through the response so a successful stop cannot race
        // an accepted save, job start, or other document operation.
        let _admission = if matches!(request.body, RequestBody::Session { .. }) {
            Some(exit.admit()?)
        } else {
            None
        };
        let (body, close) = match request.body {
            RequestBody::Lifecycle { action } if ready => {
                let locked = sessions
                    .lock()
                    .map_err(|_| ApiError::new(ErrorCode::IoError, "終了確認のロック障害"))?;
                (
                    ResponseBody::Lifecycle {
                        status: exit.inspect(&connection, &locked, action),
                    },
                    false,
                )
            }
            RequestBody::Watch { after, wait_ms } if ready => (
                match tokio::select! {
                    result = crate::events::wait(&sessions, after, wait_ms) => result,
                    _ = closing.changed() => return Ok(()),
                    // A disconnected desktop must not keep its watch alive for 20s.
                    _ = disconnected(&stream) => return Ok(()),
                } {
                    Ok(batch) => ResponseBody::Events { batch },
                    Err(error) => ResponseBody::Error { error },
                },
                false,
            ),
            RequestBody::Session { request }
                if ready && crate::voice_catalog::handles(&request) =>
            {
                (
                    match crate::voice_catalog::execute(&sessions, request).await {
                        Ok(result) => ResponseBody::Session { result },
                        Err(error) => ResponseBody::Error { error },
                    },
                    false,
                )
            }
            RequestBody::Session { request } if ready => {
                let sessions = sessions.clone();
                let result =
                    tokio::task::spawn_blocking(move || Sessions::execute(&sessions, request))
                        .await
                        .map_err(ApiError::io)?;
                (
                    match result {
                        Ok(result) => ResponseBody::Session { result },
                        Err(error) => ResponseBody::Error { error },
                    },
                    false,
                )
            }
            body => dispatch(body, &mut ready, info),
        };
        timeout(
            Duration::from_secs(2),
            write_frame(
                &mut stream,
                &Response {
                    request_id: request.request_id,
                    body,
                },
            ),
        )
        .await
        .map_err(|_| ApiError::new(ErrorCode::HostUnavailable, "応答待機時間を超えました"))??;
        if close {
            return Ok(());
        }
    }
}

fn dispatch(request: RequestBody, ready: &mut bool, info: &HostInfo) -> (ResponseBody, bool) {
    match request {
        RequestBody::Hello { hello } if !*ready => {
            if hello.application != APPLICATION
                || hello.protocol_major != PROTOCOL_MAJOR
                || hello.protocol_minor < 33
            {
                return (
                    ResponseBody::Error {
                        error: ApiError::new(
                            ErrorCode::ProtocolMismatch,
                            "互換性のないホストプロトコルです。Sungekiのビルドを揃えてください",
                        ),
                    },
                    true,
                );
            }
            *ready = true;
            (ResponseBody::Hello { host: info.clone() }, false)
        }
        RequestBody::GetCapabilities {} if *ready => {
            (ResponseBody::Capabilities { host: info.clone() }, false)
        }
        _ => (
            ResponseBody::Error {
                error: ApiError::new(
                    ErrorCode::ValidationFailed,
                    "接続の最初に一度だけhelloが必要です",
                ),
            },
            true,
        ),
    }
}

async fn disconnected(stream: &UnixStream) {
    let mut byte = [0];
    loop {
        if stream.readable().await.is_err() {
            return;
        }
        match stream.try_read(&mut byte) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            _ => return, // EOF or unsupported pipelining while a watch is pending.
        }
    }
}
