//! Read-only native visibility diagnostics; never changes activation or window state.
#[cfg(target_os = "macos")]
pub(super) fn record(window: &tauri::WebviewWindow) {
    let owned = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Some(main_thread) = objc2::MainThreadMarker::new() else { return };
        let application = objc2_app_kit::NSApplication::sharedApplication(main_thread);
        let Ok(pointer) = owned.ns_window() else { return };
        // Tauri owns this NSWindow. Borrow only on its main thread while the
        // owned WebviewWindow handle is alive; never retain or mutate the pointer.
        let Some(native) = (unsafe { pointer.cast::<objc2_app_kit::NSWindow>().as_ref() }) else {
            return;
        };
        super::record(&format!(
            "native.visibility visible={} key={} minimized={} occlusion_visible={} app_active={} app_hidden={}",
            native.isVisible(), native.isKeyWindow(), native.isMiniaturized(),
            native.occlusionState().contains(objc2_app_kit::NSWindowOcclusionState::Visible),
            application.isActive(), application.isHidden()
        ));
    });
}

#[cfg(not(target_os = "macos"))]
pub(super) fn record(_window: &tauri::WebviewWindow) {}
