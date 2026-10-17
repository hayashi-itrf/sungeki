//! Native quit interception; the common host decides whether stopping is safe.
use std::os::unix::fs::MetadataExt;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use sungeki_host_client::{HostClient, HostOptions};
use sungeki_protocol::{ApiError, ErrorCode, ExitAction, ExitStatus};
use tauri::{Emitter, Manager, RunEvent};
use tokio::sync::{Mutex, RwLock};

static APPROVED: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static STOPPING: AtomicBool = AtomicBool::new(false);
static STOP_INODE: OnceLock<Option<u64>> = OnceLock::new();
static GATE: RwLock<()> = RwLock::const_new(());
static LEASE: OnceLock<Mutex<Option<HostClient>>> = OnceLock::new();
fn lease() -> &'static Mutex<Option<HostClient>> {
    LEASE.get_or_init(|| Mutex::new(None))
}
fn error(message: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::HostUnavailable, message)
}

pub async fn allow_request() -> Result<tokio::sync::RwLockReadGuard<'static, ()>, ApiError> {
    let guard = GATE.read().await;
    if STOPPING.load(Ordering::SeqCst) {
        return Err(error("終了処理中です"));
    }
    Ok(guard)
}
pub fn start_lease() {
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if STOPPING.load(Ordering::SeqCst) {
                continue;
            }
            let Ok(options) = HostOptions::from_environment() else {
                continue;
            };
            let mut held = lease().lock().await;
            if held.is_none() {
                *held = HostClient::connect_existing(&options, "Desktop lifetime")
                    .await
                    .ok()
                    .flatten();
            }
            if let Some(client) = held.as_mut() {
                if client.capabilities().await.is_err() {
                    *held = None;
                }
            }
        }
    });
}
#[tauri::command]
pub fn exit_ready() {
    READY.store(true, Ordering::SeqCst);
}

pub fn on_event(app: &tauri::AppHandle, event: RunEvent) {
    let requested = match event {
        RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } if !APPROVED.load(Ordering::SeqCst) && READY.load(Ordering::SeqCst) => {
            api.prevent_close();
            true
        }
        RunEvent::ExitRequested { api, .. }
            if !APPROVED.load(Ordering::SeqCst) && READY.load(Ordering::SeqCst) =>
        {
            api.prevent_exit();
            true
        }
        _ => false,
    };
    if requested {
        request_quit(app);
    }
}
pub fn can_terminate() -> bool {
    APPROVED.load(Ordering::SeqCst) || !READY.load(Ordering::SeqCst)
}
pub fn request_quit(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit("app-quit-requested", ());
}

#[tauri::command]
pub async fn exit_status() -> Result<ExitStatus, ApiError> {
    if STOPPING.load(Ordering::SeqCst) {
        return Ok(ExitStatus {
            stopping: true,
            ..Default::default()
        });
    }
    let options = HostOptions::from_environment()?;
    match HostClient::connect_existing(&options, "Desktop exit").await? {
        Some(mut client) => client.lifecycle(ExitAction::Inspect).await,
        None => Ok(ExitStatus::default()),
    }
}
#[tauri::command]
pub async fn finish_exit(
    app: tauri::AppHandle,
    background: bool,
    cancel_jobs: bool,
) -> Result<(), ApiError> {
    // Wait for admitted native operations; nothing can restart a host after this point.
    let _gate = tokio::time::timeout(Duration::from_secs(3), GATE.write())
        .await
        .map_err(|_| {
            error("操作がまだ完了していません。編集に戻るか、少し待って終了を再試行してください")
        })?;
    let options = HostOptions::from_environment()?;
    let mut held = lease().lock().await;
    let mut client = if STOPPING.load(Ordering::SeqCst) {
        None
    } else {
        match HostClient::connect_existing(&options, "Desktop exit").await {
            Ok(client) => client,
            Err(_) if background => None, // Closing just this GUI never cancels work.
            Err(e) => return Err(e),
        }
    };
    let wait_for_host = client.is_some() || STOPPING.load(Ordering::SeqCst);
    let socket_inode = std::fs::metadata(options.socket()).ok().map(|m| m.ino());
    if let Some(client) = client.as_mut() {
        if background {
            let _ = client.lifecycle(ExitAction::WhenIdle).await;
        } else {
            let status = client
                .lifecycle(if cancel_jobs {
                    ExitAction::CancelAndStop
                } else {
                    ExitAction::StopIfIdle
                })
                .await?;
            if !status.stopping {
                return Err(error(
                    "別の画面・AIが使用中、または操作の完了待ちです。処理を続けて画面だけ終了するか、少し待って再確認してください",
                ));
            }
        }
    }
    let socket_inode = *STOP_INODE.get_or_init(|| socket_inode);
    STOPPING.store(true, Ordering::SeqCst);
    *held = None;
    drop(client);
    drop(held);
    if !background && wait_for_host {
        let done = tokio::time::timeout(Duration::from_secs(60), async {
            while socket_inode.is_some()
                && std::fs::metadata(options.socket()).ok().map(|m| m.ino()) == socket_inode
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        if done.is_err() {
            let mut e = error(
                "後始末が続いています。終了を再試行するか、処理を続けて画面だけ終了できます。強制終了はしていません",
            );
            e.details.insert("exit_stopping".into(), "true".into());
            return Err(e);
        }
    }
    APPROVED.store(true, Ordering::SeqCst);
    app.exit(0);
    Ok(())
}

pub async fn retain_connection(options: &HostOptions) -> Result<(), ApiError> {
    let mut held = lease().lock().await;
    if held.is_none() {
        *held = HostClient::connect_existing(options, "Desktop lifetime").await?;
    }
    Ok(())
}
