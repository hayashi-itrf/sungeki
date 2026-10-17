//! Opt-in, bounded startup diagnostics. No document contents or paths are logged.
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;
use tauri::{Builder, Manager, WindowEvent, Wry};

#[path = "startup_visibility.rs"]
mod visibility;

static START: OnceLock<Instant> = OnceLock::new();
static REMAINING: AtomicUsize = AtomicUsize::new(64);
static LAST_WINDOW_EVENT: Mutex<Option<String>> = Mutex::new(None);

pub fn configure(builder: Builder<Wry>) -> Builder<Wry> {
    if std::env::var("SUNGEKI_STARTUP_TRACE").as_deref() != Ok("1") {
        return builder;
    }
    START.get_or_init(Instant::now);
    record("native.start");
    builder
        .append_invoke_initialization_script(include_str!("startup_trace.js"))
        .on_page_load(|window, payload| {
            record(&format!(
                "page.{:?} inner_size={:?} scale={:?} webview_bounds={:?}",
                payload.event(),
                window.window().inner_size(),
                window.window().scale_factor(),
                window.bounds()
            ));
        })
        .on_window_event(|window, event| {
            let stage = match event {
                WindowEvent::Focused(true) => "window.focused",
                WindowEvent::Focused(false) => "window.unfocused",
                WindowEvent::Resized(_) => "window.resized",
                WindowEvent::Destroyed => "window.destroyed",
                _ => return,
            };
            let message = format!(
                "{stage} visible={:?} minimized={:?} inner_size={:?} scale={:?}",
                window.is_visible(),
                window.is_minimized(),
                window.inner_size(),
                window.scale_factor()
            );
            // macOS can emit identical resize notifications in a burst.
            // Preserve the bounded log for later visibility and frame events.
            let Ok(mut previous) = LAST_WINDOW_EVENT.lock() else {
                return;
            };
            if previous.as_deref() != Some(message.as_str()) {
                record(&message);
                *previous = Some(message);
                // Visibility callbacks may not fire when a background app's window
                // becomes key. Capture native state at that boundary as well.
                if let Some(webview) = window.get_webview_window(window.label()) {
                    visibility::record(&webview);
                }
            }
        })
}

#[tauri::command]
pub fn trace_startup(
    window: tauri::WebviewWindow,
    stage: String,
    hidden: bool,
    nodes: u32,
    viewport_width: u32,
    viewport_height: u32,
    app_width: u32,
    app_height: u32,
    pixel_ratio: f64,
) {
    if matches!(
        stage.as_str(),
        "entry" | "mounted" | "failed" | "dom_ready" | "visibility" | "frame" | "layout" | "resize"
    ) {
        record(&format!(
            "js.{stage} hidden={hidden} app_children={nodes} viewport={viewport_width}x{viewport_height} app={app_width}x{app_height} pixel_ratio={pixel_ratio}"
        ));
        if START.get().is_some() && matches!(stage.as_str(), "layout" | "resize" | "visibility") {
            visibility::record(&window);
        }
    }
}

fn record(message: &str) {
    let Some(start) = START.get() else { return };
    if REMAINING
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
            remaining.checked_sub(1)
        })
        .is_ok()
    {
        eprintln!(
            "[next-startup +{}ms] {message}",
            start.elapsed().as_millis()
        );
    }
}
